use crate::config::AggConfig;
use crate::domain::{Aggregate, Dispatch, EventEnum, Load, Replay, Stream};
use crate::errors::DomainError;
use crate::pool::{BufferGuard, BufferPool, BufferPoolHandler};
use crate::{BINCODE_CONFIG, EMPTY_BYTES};
use ahash::{AHashMap, AHashSet, AHasher};
use bincode::error::EncodeError;
use bytes::{Bytes, BytesMut};
use std::{collections::VecDeque, hash::Hasher, marker::PhantomData, sync::Arc};
use tokio::{
    sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub enum AggTask {
    Com {
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
        _permit: OwnedSemaphorePermit,
    },
}

#[repr(u8)]
#[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
pub enum Res {
    Success = 0,
    Duplicate = 1,
    Fail = 2,
}

impl<A, E, L, R, S, F> Dispatch<A, E, L, R, S> for F
where
    A: Aggregate,
    E: EventEnum<A = A>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream,
    F: FnMut(
        &'static str,
        Uuid,
        Vec<u8>,
        &mut AHashMap<Uuid, (A, Instant)>,
        &L,
        &R,
    ) -> Result<((A, Instant), A, E), DomainError>,
{
    #[inline(always)]
    fn dispatch(
        &mut self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut AHashMap<Uuid, (A, Instant)>,
        loader: &L,
        replayer: &R,
    ) -> Result<((A, Instant), A, E), DomainError> {
        self(agg_type, agg_id, com_data, caches, loader, replayer)
    }
}

impl<A, R, S, F> Load<A, R, S> for F
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream,
    F: Fn(
        &'static str,
        Uuid,
        &mut AHashMap<Uuid, (A, Instant)>,
        &R,
    ) -> Result<(A, Instant), DomainError>,
{
    #[inline(always)]
    fn load(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        caches: &mut AHashMap<Uuid, (A, Instant)>,
        replayer: &R,
    ) -> Result<(A, Instant), DomainError> {
        self(agg_type, agg_id, caches, replayer)
    }
}

pub struct Aggregator<A, D, E, L, R, S>
where
    A: Aggregate,
    D: Dispatch<A, E, L, R, S>,
    E: EventEnum<A = A>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream,
{
    concurrent: usize,
    hasher: AHasher,
    senders: Vec<mpsc::Sender<AggTask>>,
    semaphore: Arc<Semaphore>,
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L, R, S> Aggregator<A, D, E, L, R, S>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, E, L, R, S> + Send + Copy + 'static,
    E: EventEnum<A = A> + bincode::Encode + Send + 'static,
    L: Load<A, R, S> + Send + Copy + 'static,
    R: Replay<A = A> + Send + Copy + 'static,
    S: Stream + Send + Copy + 'static,
{
    async fn processor(
        idx: u64,
        cfg: AggConfig,
        pool: Arc<Mutex<BufferPool>>,
        buf_tx: mpsc::Sender<BytesMut>,
        mut agg_rx: mpsc::Receiver<AggTask>,
        mut dispatcher: D,
        loader: L,
        replayer: R,
    ) {
        let agg_type = std::any::type_name::<A>();
        let start = Instant::now() + Duration::from_secs(idx);
        let mut interval = interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut caches: AHashMap<Uuid, (A, Instant)> = AHashMap::with_capacity(cfg.high);
        let mut com_set: AHashSet<Uuid> = AHashSet::new();
        let mut com_vec: VecDeque<Uuid> = VecDeque::new();
        S::restore(agg_type, &mut com_set, &mut com_vec, cfg.coms);

        loop {
            tokio::select! {
                biased;
                Some(task) = agg_rx.recv() => {
                    match task {
                        AggTask::Com { agg_id, com_id, com_data, _permit } => {
                            info!("开始处理聚合{agg_id}命令{com_id}");
                            if com_set.contains(&com_id) {
                                warn!("重复提交聚合{agg_id}命令{com_id}错误");
                                match S::respond(agg_type, agg_id, com_id, Res::Duplicate, &EMPTY_BYTES).await {
                                    Ok(()) => info!("重复提交聚合{agg_id}命令{com_id}错误反馈成功"),
                                    Err(e) => warn!("重复提交聚合{agg_id}命令{com_id}错误反馈失败：{e}"),
                                }
                            } else {
                                match dispatcher.dispatch(agg_type, agg_id, com_data, &mut caches, &loader, &replayer) {
                                    Ok(((oa, ot), mut na, evt)) => {
                                        debug!("聚合{agg_id}命令{com_id}预处理成功");
                                        let mut buf = BufferGuard::get(pool.clone());
                                        match loop {
                                            match bincode::encode_into_slice(&evt, buf.as_mut(), BINCODE_CONFIG) {
                                                Ok(len) => break Ok(&buf[..len]),
                                                Err(EncodeError::UnexpectedEnd) => {
                                                    let new_size = buf.capacity() * 2;
                                                    buf.reserve(new_size);
                                                }
                                                Err(e) => break Err(DomainError::EncodeError(e)),
                                            }
                                        } {
                                            Ok(bytes) => match S::write(agg_type, agg_id, com_id, na.revision(), bytes).await {
                                                Ok(()) => {
                                                    info!("聚合{agg_id}命令{com_id}写入成功");
                                                    na.next();
                                                    caches.insert(agg_id, (na, Instant::now()));
                                                }
                                                Err(e) => {
                                                    warn!("聚合{agg_id}命令{com_id}写入失败：{e}");
                                                    let evt_data = Bytes::from(e.to_string());
                                                    match S::respond(agg_type, agg_id, com_id, Res::Fail, &evt_data).await {
                                                        Ok(()) => info!("聚合{agg_id}命令{com_id}写入失败反馈成功"),
                                                        Err(e) => warn!("聚合{agg_id}命令{com_id}写入失败反馈失败：{e}"),
                                                    }
                                                    if oa.revision() != u64::MAX {
                                                        caches.insert(agg_id, (oa, ot));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("聚合{agg_id}命令{com_id}事件序列化错误：{e}");
                                                let evt_data = Bytes::from(e.to_string());
                                                match S::respond(agg_type, agg_id, com_id, Res::Fail, &evt_data).await {
                                                    Ok(()) => info!("聚合{agg_id}命令{com_id}事件序列化错误反馈成功"),
                                                    Err(e) => warn!("聚合{agg_id}命令{com_id}事件序列化错误反馈失败：{e}"),
                                                }
                                            }
                                        }
                                        buf_tx.send(buf).await.ok();
                                    },
                                    Err(e) => {
                                        warn!("聚合{agg_id}命令{com_id}预处理错误：{e}");
                                        let evt_data = Bytes::from(e.to_string());
                                        match S::respond(agg_type, agg_id, com_id, Res::Fail, &evt_data).await {
                                            Ok(()) => info!("聚合{agg_id}命令{com_id}预处理错误反馈成功"),
                                            Err(e) => warn!("聚合{agg_id}命令{com_id}预处理错误反馈失败：{e}"),
                                        }
                                    },
                                }
                                com_vec.push_back(com_id);
                                com_set.insert(com_id);
                            }
                        },
                    }
                }
                _ = interval.tick() => {
                    match caches.len() {
                        len if len <= cfg.low => (),
                        len if len > cfg.high => {
                            let mut retain = cfg.retain;
                            caches.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(retain));
                            while caches.len() > cfg.high {
                                retain = retain / 2;
                                caches.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(retain));
                            }
                        },
                        _ => {
                            caches.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(cfg.retain));
                        }
                    }
                    let len = com_vec.len() - cfg.coms;
                    if len > 0 {
                        let mut drained = AHashSet::with_capacity(len);
                        drained.extend(com_vec.drain(0..len));
                        com_set.extract_if(|id| drained.contains(id));
                    }
                }
            }
        }
    }

    pub async fn new(cfg: AggConfig, dispatcher: D, loader: L, replayer: R) -> Self {
        let pool = Arc::new(Mutex::new(BufferPool::new(cfg.sems, 1024, 8192)));
        let (buf_tx, buf_rx) = mpsc::channel::<BytesMut>(cfg.sems);
        let semaphore = Arc::new(Semaphore::new(cfg.sems));
        let mut senders: Vec<mpsc::Sender<AggTask>> = Vec::with_capacity(cfg.concurrent);
        let handler = BufferPoolHandler {
            inner: pool.clone(),
            buf_rx,
        };

        tokio::spawn(handler.run());
        for idx in 0..cfg.concurrent {
            let (tx, rx) = mpsc::channel::<AggTask>(cfg.capacity);
            let idx = idx.try_into().unwrap();
            senders.push(tx);
            tokio::spawn(Self::processor(
                idx,
                cfg.clone(),
                pool.clone(),
                buf_tx.clone(),
                rx,
                dispatcher,
                loader,
                replayer,
            ));
        }

        Self {
            concurrent: cfg.concurrent,
            hasher: AHasher::default(),
            senders,
            semaphore,
            _marker_d: PhantomData,
            _marker_e: PhantomData,
            _marker_r: PhantomData,
            _marker_s: PhantomData,
            _marker_l: PhantomData,
        }
    }

    pub async fn commit(
        &mut self,
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
    ) -> Result<(), DomainError> {
        let permit = self.semaphore.clone().acquire_owned().await?;
        match self.concurrent {
            1 => self.senders[0]
                .send(AggTask::Com {
                    agg_id,
                    com_id,
                    com_data,
                    _permit: permit,
                })
                .await
                .map_err(|e| DomainError::SendError(e.to_string())),
            _ => {
                self.hasher.write(agg_id.as_bytes());
                let hash = self.hasher.finish();
                let idx = (hash % self.concurrent as u64) as usize;
                self.hasher = AHasher::default();
                self.senders[idx]
                    .send(AggTask::Com {
                        agg_id,
                        com_id,
                        com_data,
                        _permit: permit,
                    })
                    .await
                    .map_err(|e| DomainError::SendError(e.to_string()))
            }
        }
    }
}

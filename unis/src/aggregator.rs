use crate::config::AggConfig;
use crate::domain::{Aggregate, Dispatch, EventEnum, Load, Restore, Write};
use crate::errors::DomainError;
use crate::pool::BufferPool;
use crate::{BINCODE_CONFIG, EMPTY_BYTES};
use ahash::{AHashMap, AHashSet};
use bincode::error::EncodeError;
use bytes::Bytes;
use std::{collections::VecDeque, marker::PhantomData, sync::Arc};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{debug, error, info, warn};
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

impl<F, Fut> Restore for F
where
    F: Fn(&'static str, i64) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(AHashSet<Uuid>, VecDeque<Uuid>), DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut {
        self(agg_type, latest)
    }
}

impl<F, Fut> Load for F
where
    F: Fn(&'static str, Uuid) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Vec<Vec<u8>>, DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn load(&self, agg_type: &'static str, agg_id: Uuid) -> Self::Fut {
        self(agg_type, agg_id)
    }
}

impl<A, E, L, F, Fut> Dispatch<A, E, L> for F
where
    A: Aggregate,
    E: EventEnum<A = A>,
    L: Load,
    F: Fn(&'static str, Uuid, Vec<u8>, Option<(A, Instant)>, L) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<((A, Instant), A, E), DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline(always)]
    fn dispatch(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_data: Vec<u8>,
        agg: Option<(A, Instant)>,
        loader: L,
    ) -> Self::Fut {
        self(agg_type, agg_id, com_data, agg, loader)
    }
}

impl<F, Fut> Write for F
where
    F: Fn(&'static str, Uuid, Uuid, u64, Res, Bytes) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn write(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        res: Res,
        evt_data: Bytes,
    ) -> Self::Fut {
        self(agg_type, agg_id, com_id, revision, res, evt_data)
    }
}

pub struct Aggregator<A, D, E, L>
where
    A: Aggregate,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A>,
    L: Load,
{
    agg_tx: mpsc::Sender<AggTask>,
    semaphore: Arc<Semaphore>,
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L> Aggregator<A, D, E, L>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A> + bincode::Encode + Send + 'static,
    L: Load + Copy,
{
    async fn processor(
        cfg: AggConfig,
        pool: BufferPool,
        mut agg_rx: mpsc::Receiver<AggTask>,
        dispatcher: D,
        loader: L,
        restorer: impl Restore,
        writer: impl Write,
    ) {
        let agg_type = std::any::type_name::<A>();
        let start = Instant::now();
        let mut interval = interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut caches: AHashMap<Uuid, (A, Instant)> = AHashMap::with_capacity(cfg.high);
        let latest = cfg.latest;

        let (mut com_set, mut com_vec) = match restorer.restore(agg_type, latest).await {
            Ok(coms) => coms,
            Err(e) => {
                error!("恢复聚合{agg_type}命令操作记录失败：{e}");
                panic!("恢复命令操作记录失败");
            }
        };

        info!("成功恢复聚合{agg_type}最近{latest}分钟的命令操作记录");

        loop {
            tokio::select! {
                biased;
                Some(task) = agg_rx.recv() => {
                    match task {
                        AggTask::Com { agg_id, com_id, com_data, _permit } => {
                            info!("开始处理聚合{agg_id}命令{com_id}");
                            if com_set.contains(&com_id) {
                                warn!("重复提交聚合{agg_id}命令{com_id}错误");
                                match writer.write(agg_type, agg_id, com_id, 0, Res::Duplicate, EMPTY_BYTES).await {
                                    Ok(()) => info!("重复提交聚合{agg_id}命令{com_id}错误反馈成功"),
                                    Err(e) => warn!("重复提交聚合{agg_id}命令{com_id}错误反馈失败：{e}"),
                                }
                            } else {
                                match dispatcher.dispatch(agg_type, agg_id, com_data, caches.remove(&agg_id), loader).await {
                                    Ok(((oa, ot), mut na, evt)) => {
                                        debug!("聚合{agg_id}命令{com_id}预处理成功");
                                        let mut buf = pool.get();
                                        match loop {
                                            match bincode::encode_into_slice(&evt, buf.as_mut(), BINCODE_CONFIG) {
                                                Ok(len) => break Ok(buf.split_to(len).freeze()),
                                                Err(EncodeError::UnexpectedEnd) => {
                                                    let new_size = buf.capacity() * 2;
                                                    buf.reserve(new_size);
                                                }
                                                Err(e) => break Err(DomainError::EncodeError(e)),
                                            }
                                        } {
                                            Ok(bytes) => match writer.write(agg_type, agg_id, com_id, na.revision(), Res::Success, bytes).await {
                                                Ok(()) => {
                                                    info!("聚合{agg_id}命令{com_id}写入成功");
                                                    na.next();
                                                    caches.insert(agg_id, (na, Instant::now()));
                                                }
                                                Err(e) => {
                                                    warn!("聚合{agg_id}命令{com_id}写入失败：{e}");
                                                    let evt_data = Bytes::from(e.to_string());

                                                    match writer.write(agg_type, agg_id, com_id, 0, Res::Fail, evt_data).await {
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
                                                match writer.write(agg_type, agg_id, com_id, 0, Res::Fail, evt_data).await {
                                                    Ok(()) => info!("聚合{agg_id}命令{com_id}事件序列化错误反馈成功"),
                                                    Err(e) => warn!("聚合{agg_id}命令{com_id}事件序列化错误反馈失败：{e}"),
                                                }
                                            }
                                        }
                                        pool.put(buf);
                                    },
                                    Err(e) => {
                                        warn!("聚合{agg_id}命令{com_id}预处理错误：{e}");
                                        let evt_data = Bytes::from(e.to_string());
                                        match writer.write(agg_type, agg_id, com_id, 0, Res::Fail, evt_data).await {
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

    pub async fn new(
        cfg: AggConfig,
        dispatcher: D,
        loader: L,
        restorer: impl Restore,
        writer: impl Write,
    ) -> Self {
        let pool = BufferPool::new(4096, cfg.sems);
        let semaphore = Arc::new(Semaphore::new(cfg.sems));
        let (agg_tx, agg_rx) = mpsc::channel::<AggTask>(cfg.capacity);
        tokio::spawn(Self::processor(
            cfg.clone(),
            pool.clone(),
            agg_rx,
            dispatcher,
            loader,
            restorer,
            writer,
        ));

        Self {
            agg_tx,
            semaphore,
            _marker_d: PhantomData,
            _marker_e: PhantomData,
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
        self.agg_tx
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

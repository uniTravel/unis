//! 聚合器
//!
//!

use crate::BINCODE_CONFIG;
use crate::config::AggConfig;
use crate::domain::{Aggregate, Dispatch, EventEnum, Load, Replay, Stream};
use crate::errors::DomainError;
use crate::pool::{BufferGuard, BufferPool, BufferPoolHandler};
use ahash::{AHashMap, AHasher};
use bincode::error::EncodeError;
use bytes::{Bytes, BytesMut};
use std::{hash::Hasher, marker::PhantomData, sync::Arc};
use tokio::{
    sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc},
    time::{Duration, Instant},
};
use tracing::{debug, warn};
use uuid::Uuid;

/// 聚合器任务
pub enum AggTask {
    /// 命令
    Com {
        /// 聚合Id
        agg_id: Uuid,
        /// 命令Id
        com_id: Uuid,
        /// 命令数据
        com_data: Vec<u8>,
        /// 回复发送通道
        _permit: OwnedSemaphorePermit,
    },
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
        &S,
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
        stream: &S,
    ) -> Result<((A, Instant), A, E), DomainError> {
        self(agg_type, agg_id, com_data, caches, loader, replayer, stream)
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
        &S,
    ) -> Result<(A, Instant), DomainError>,
{
    #[inline(always)]
    fn load(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        caches: &mut AHashMap<Uuid, (A, Instant)>,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, Instant), DomainError> {
        self(agg_type, agg_id, caches, replayer, stream)
    }
}

/// 泛型的聚合加载函数
///
/// 构造聚合器时，具体化的聚合加载函数作为参数，系统会自动将其转为闭包传入。
pub fn loader<A, R, S>(
    agg_type: &'static str,
    agg_id: Uuid,
    caches: &mut AHashMap<Uuid, (A, Instant)>,
    replayer: &R,
    stream: &S,
) -> Result<(A, Instant), DomainError>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream,
{
    if let Some(o) = caches.remove(&agg_id) {
        return Ok(o);
    } else {
        let mut oa = A::new(agg_id);
        let ds = stream.read(agg_type, agg_id)?;
        for evt_data in ds {
            replayer.replay(&mut oa, evt_data)?;
        }
        Ok((oa, Instant::now()))
    }
}

/// 聚合器
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
        cfg: AggConfig,
        pool: Arc<Mutex<BufferPool>>,
        buf_tx: mpsc::Sender<BytesMut>,
        mut agg_rx: mpsc::Receiver<AggTask>,
        mut dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) {
        let agg_type = std::any::type_name::<A>();
        let mut interval = tokio::time::interval(Duration::from_secs(cfg.interval));
        let mut caches: AHashMap<Uuid, (A, Instant)> = AHashMap::with_capacity(cfg.high);
        // TODO：崩溃重启，消息重复，都会导致重复消费，需要检查命令是否已经处理

        loop {
            tokio::select! {
                biased;
                Some(task) = agg_rx.recv() => {
                    match task {
                        AggTask::Com { agg_id, com_id, com_data, _permit } => {
                            match dispatcher.dispatch(agg_type, agg_id, com_data, &mut caches, &loader, &replayer, &stream) {
                                Ok(((oa, ot), mut na, evt)) => {
                                    let mut guard = BufferGuard::new(pool.clone(), buf_tx.clone());
                                    match loop {
                                        match bincode::encode_into_slice(&evt, guard.buf.as_mut().unwrap(), BINCODE_CONFIG) {
                                            Ok(len) => break Ok(guard.into_inner().split_to(len).freeze()),
                                            Err(EncodeError::UnexpectedEnd) => {
                                                let new_size = guard.buf.as_ref().unwrap().capacity() * 2;
                                                guard.buf.as_mut().unwrap().reserve(new_size);
                                            }
                                            Err(e) => break Err(DomainError::EncodeError(e)),
                                        }
                                    } {
                                        Ok(bytes) => match stream.write(agg_type, agg_id, com_id, na.revision(), bytes, buf_tx.clone()).await {
                                            Ok(()) => {
                                                debug!("聚合{agg_id}命令{com_id}写入成功");
                                                na.next();
                                                caches.insert(agg_id, (na, Instant::now()));
                                            }
                                            Err(e) => {
                                                warn!("写入聚合{agg_id}命令{com_id}错误：{e}");
                                                if oa.revision() != u64::MAX {
                                                    caches.insert(agg_id, (oa, ot));
                                                }
                                            }
                                        }
                                        Err(e) => warn!("写入聚合{agg_id}命令{com_id}错误：{e}"),
                                    }
                                },
                                Err(e) => {
                                    let evt_data = Bytes::from(e.to_string());
                                    match stream.fail(agg_type, agg_id, com_id, evt_data).await {
                                        Ok(()) => debug!("聚合{agg_id}命令{com_id}处理异常写入成功"),
                                        Err(e) => warn!("聚合{agg_id}命令{com_id}处理异常无法写入：{e}"),
                                    }
                                },
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
                }
            }
        }
    }

    /// 聚合器构造函数
    pub async fn new(cfg: AggConfig, dispatcher: D, loader: L, replayer: R, stream: S) -> Self {
        let pool = Arc::new(Mutex::new(BufferPool::new(cfg.sems, 1024, 8192)));
        let (buf_tx, buf_rx) = mpsc::channel::<BytesMut>(cfg.sems);
        let semaphore = Arc::new(Semaphore::new(cfg.sems));
        let mut senders: Vec<mpsc::Sender<AggTask>> = Vec::with_capacity(cfg.concurrent);
        let handler = BufferPoolHandler {
            inner: pool.clone(),
            buf_rx,
        };

        tokio::spawn(handler.run());
        for _ in 0..cfg.concurrent {
            let (tx, rx) = mpsc::channel::<AggTask>(cfg.capacity);
            senders.push(tx);
            tokio::spawn(Self::processor(
                cfg.clone(),
                pool.clone(),
                buf_tx.clone(),
                rx,
                dispatcher,
                loader,
                replayer,
                stream,
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

    /// 提交命令
    pub async fn commit(
        &mut self,
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
    ) -> Result<(), mpsc::error::SendError<AggTask>> {
        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        match self.concurrent {
            1 => {
                self.senders[0]
                    .send(AggTask::Com {
                        agg_id,
                        com_id,
                        com_data,
                        _permit: permit,
                    })
                    .await
            }
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
            }
        }
    }
}

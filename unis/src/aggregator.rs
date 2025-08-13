//! 聚合器
//!
//!

use crate::BINCODE_CONFIG;
use crate::config::AggConfig;
use crate::domain::{Aggregate, Dispatch, EventEnum, Load, Replay, Stream};
use crate::errors::DomainError;
use crate::pool::{BufferGuard, BufferPool, BufferPoolHandler};
use bincode::error::EncodeError;
use bytes::{Bytes, BytesMut};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::{Duration, Instant},
};
use tracing::error;
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
        reply_tx: oneshot::Sender<Result<(), DomainError>>,
    },
    /// 异常事件
    Evt {
        /// 聚合Id
        agg_id: Uuid,
        /// 命令Id
        com_id: Uuid,
        /// 命令执行结果
        res: Res,
        /// 事件数据
        evt_data: Bytes,
    },
}

/// 命令执行结果
#[repr(u8)]
pub enum Res {
    /// 成功
    Success = 0,
    /// 重复提交
    Duplicate = 1,
    /// 失败
    Fail = 2,
}

impl<A, E, L, R, S, F> Dispatch<A, E, L, R, S> for F
where
    A: Aggregate,
    E: EventEnum<A = A>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
    F: FnMut(
        Uuid,
        Vec<u8>,
        &mut HashMap<Uuid, (A, Instant)>,
        &L,
        &R,
        &S,
    ) -> Result<((A, Instant), A, E), DomainError>,
{
    #[inline(always)]
    fn dispatch(
        &mut self,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, (A, Instant)>,
        loader: &L,
        replayer: &R,
        stream: &S,
    ) -> Result<((A, Instant), A, E), DomainError> {
        self(agg_id, com_data, caches, loader, replayer, stream)
    }
}

impl<A, R, S, F> Load<A, R, S> for F
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
    F: Fn(Uuid, &mut HashMap<Uuid, (A, Instant)>, &R, &S) -> Result<(A, Instant), DomainError>,
{
    #[inline(always)]
    fn load(
        &self,
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, (A, Instant)>,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, Instant), DomainError> {
        self(agg_id, caches, replayer, stream)
    }
}

/// 泛型的聚合加载函数
///
/// 构造聚合器时，具体化的聚合加载函数作为参数，系统会自动将其转为闭包传入。
pub fn loader<A, R, S>(
    agg_id: Uuid,
    caches: &mut HashMap<Uuid, (A, Instant)>,
    replayer: &R,
    stream: &S,
) -> Result<(A, Instant), DomainError>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    if let Some(o) = caches.remove(&agg_id) {
        return Ok(o);
    } else {
        let mut oa = A::new(agg_id);
        let ds = stream.read(agg_id)?;
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
    S: Stream<A = A>,
{
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L, R, S> Aggregator<A, D, E, L, R, S>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, E, L, R, S> + Send + 'static,
    E: EventEnum<A = A> + bincode::Encode + Send + 'static,
    L: Load<A, R, S> + Send + 'static,
    R: Replay<A = A> + Send + 'static,
    S: Stream<A = A> + Send + 'static,
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
        let mut interval = tokio::time::interval(Duration::from_secs(cfg.interval));
        let mut caches: HashMap<Uuid, (A, Instant)> = HashMap::with_capacity(cfg.high);
        // TODO：崩溃重启，消息重复，都会导致重复消费，需要检查命令是否已经处理

        loop {
            tokio::select! {
                biased;
                Some(task) = agg_rx.recv() => {
                    match task {
                        AggTask::Com { agg_id, com_id, com_data, reply_tx } => {
                            match dispatcher.dispatch(agg_id, com_data, &mut caches, &loader, &replayer, &stream) {
                                Ok(((oa, ot), mut na, evt)) => {
                                    let mut guard = BufferGuard::new(pool.clone(), buf_tx.clone());
                                    match loop {
                                        match bincode::encode_into_slice(&evt, guard.buf.as_mut().unwrap(), BINCODE_CONFIG) {
                                            Ok(len) => break Ok(guard.into_inner().split_to(len).freeze()),
                                            Err(EncodeError::UnexpectedEnd) => {
                                                let new_size = guard.buf.as_ref().unwrap().capacity() * 2;
                                                guard.buf.as_mut().unwrap().reserve(new_size);
                                            }
                                            Err(e) => break Err(DomainError::EncodeError(e))
                                        }
                                    } {
                                        Ok(bytes) => match stream.write(agg_id, com_id, na.revision(), bytes, buf_tx.clone()).await {
                                            Ok(()) => {
                                                let _ = reply_tx.send(Ok(()));
                                                na.next();
                                                caches.insert(agg_id, (na, Instant::now()));
                                            }
                                            Err(e) => {
                                                let _ = reply_tx.send(Err(e));
                                                if oa.revision() != u64::MAX {
                                                    caches.insert(agg_id, (oa, ot));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let _ = reply_tx.send(Err(e));
                                        }
                                    }
                                },
                                Err(e) => {
                                    reply_tx.send(Err(e)).ok();
                                },
                            }
                        },
                        AggTask::Evt { agg_id, com_id, res, evt_data } => {
                            if let Err(e) = stream.respond(agg_id, com_id, res, evt_data).await {
                                error!("聚合{agg_id}命令{com_id}处理异常无法写入：{e}");
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
    pub async fn new(
        cfg: AggConfig,
        agg_rx: mpsc::Receiver<AggTask>,
        dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) -> Self {
        let (buf_tx, buf_rx) = mpsc::channel::<BytesMut>(cfg.capacity);
        let pool = Arc::new(Mutex::new(BufferPool::new(cfg.capacity, 1024, 8192)));
        let handler = BufferPoolHandler {
            inner: pool.clone(),
            buf_rx,
        };

        tokio::spawn(handler.run());
        tokio::spawn(Self::processor(
            cfg, pool, buf_tx, agg_rx, dispatcher, loader, replayer, stream,
        ));
        Self {
            _marker_d: PhantomData,
            _marker_e: PhantomData,
            _marker_r: PhantomData,
            _marker_s: PhantomData,
            _marker_l: PhantomData,
        }
    }
}

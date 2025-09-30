//! # **unis** 聚合器
//!
//!

use crate::{
    BINCODE_CONFIG, Com, EMPTY_BYTES, Response,
    config::SubscribeConfig,
    domain::{Aggregate, Dispatch, EventEnum, Load, Restore, Stream},
    errors::UniError,
    pool::BufferPool,
};
use ahash::{AHashMap, AHashSet};
use bincode::error::EncodeError;
use bytes::Bytes;
use std::{marker::PhantomData, sync::Arc};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

impl<F, Fut> Restore for F
where
    F: Fn(&'static str, i64) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<AHashMap<Uuid, AHashSet<Uuid>>, UniError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut {
        self(agg_type, latest)
    }
}

impl<F, Fut> Load for F
where
    F: Fn(&'static str, Uuid) -> Fut + Send + Sync + Copy + 'static,
    Fut: Future<Output = Result<Vec<Vec<u8>>, UniError>> + Send + 'static,
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
    F: Fn(&'static str, Uuid, Vec<u8>, A, L) -> Fut + Send + Sync + Copy + 'static,
    Fut: Future<Output = Result<(A, E), UniError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline(always)]
    fn dispatch(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_data: Vec<u8>,
        agg: A,
        loader: L,
    ) -> Self::Fut {
        self(agg_type, agg_id, com_data, agg, loader)
    }
}

/// 聚合器结构
pub struct Aggregator<A, D, E, L>
where
    A: Aggregate,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A>,
    L: Load,
{
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L> Aggregator<A, D, E, L>
where
    A: Aggregate + Clone,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A>,
    L: Load,
{
    /// 启动聚合器
    pub async fn launch(
        cfg: &SubscribeConfig,
        dispatcher: D,
        loader: L,
        stream: Arc<impl Stream>,
        restore: impl Restore,
        mut rx: UnboundedReceiver<Com>,
    ) {
        let agg_type = std::any::type_name::<A>();
        // TODO：尚未启用信号量
        let pool = Arc::new(BufferPool::new(4096, cfg.sems));
        let latest = cfg.latest;
        let mut caches: AHashMap<Uuid, (UnboundedSender<Com>, Instant)> = AHashMap::new();
        let start = Instant::now();
        let mut interval = interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        match restore.restore(agg_type, latest).await {
            Ok(agg_coms) => {
                for (agg_id, coms) in agg_coms {
                    let (agg_tx, agg_rx) = mpsc::unbounded_channel::<Com>();
                    tokio::spawn(Self::processor(
                        agg_id,
                        agg_type,
                        pool.clone(),
                        dispatcher,
                        loader,
                        stream.clone(),
                        coms,
                        agg_rx,
                    ));
                    caches.insert(agg_id, (agg_tx, Instant::now()));
                }
            }
            Err(e) => {
                error!("恢复聚合{agg_type}命令操作记录失败：{e}");
                panic!("恢复命令操作记录失败");
            }
        }
        info!("成功恢复聚合{agg_type}最近{latest}分钟的命令操作记录");

        loop {
            tokio::select! {
                biased;
                _ = interval.tick() => {
                    match caches.len() {
                        len if len <= cfg.low => (),
                        len if len > cfg.high => {
                            let mut retain = cfg.retain;
                            let _ = caches.extract_if(|_, (_, t)| t.elapsed() > Duration::from_secs(retain));
                            while caches.len() > cfg.high {
                                retain = retain / 2;
                                let _ = caches.extract_if(|_, (_, t)| t.elapsed() > Duration::from_secs(retain));
                            }
                        },
                        _ => {
                            let _ = caches.extract_if(|_, (_, t)| t.elapsed() > Duration::from_secs(cfg.retain));
                        }
                    }
                }
                data = rx.recv() => {
                    match data {
                        Some(Com{agg_id, com_id, com_data}) => {
                            if let Some((agg_tx, instant)) = caches.get_mut(&agg_id) {
                                *instant = Instant::now();
                                if let Err(e) = agg_tx.send(Com{agg_id, com_id, com_data}) {
                                    warn!("发送聚合{agg_id}命令{com_id}失败：{e}");
                                }
                            } else {
                                let (agg_tx, agg_rx) = mpsc::unbounded_channel::<Com>();
                                tokio::spawn(Self::processor(
                                    agg_id,
                                    agg_type,
                                    pool.clone(),
                                    dispatcher,
                                    loader,
                                    stream.clone(),
                                    AHashSet::new(),
                                    agg_rx,
                                ));
                                if let Err(e) = agg_tx.send(Com{agg_id, com_id, com_data}) {
                                    warn!("发送聚合{agg_id}命令{com_id}失败：{e}");
                                }
                                caches.insert(agg_id, (agg_tx, Instant::now()));
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    }

    async fn processor(
        agg_id: Uuid,
        agg_type: &'static str,
        pool: Arc<BufferPool>,
        dispatcher: D,
        loader: L,
        stream: Arc<impl Stream>,
        mut coms: AHashSet<Uuid>,
        mut agg_rx: UnboundedReceiver<Com>,
    ) {
        let mut agg = A::new(agg_id);
        while let Some(Com {
            agg_id,
            com_id,
            com_data,
        }) = agg_rx.recv().await
        {
            info!("开始处理聚合{agg_id}命令{com_id}");
            if coms.contains(&com_id) {
                warn!("重复提交聚合{agg_id}命令{com_id}错误");
                match stream
                    .respond(agg_type, agg_id, com_id, Response::Duplicate, EMPTY_BYTES)
                    .await
                {
                    Ok(()) => {
                        info!("重复提交聚合{agg_id}命令{com_id}错误反馈成功");
                    }
                    Err(e) => {
                        warn!("重复提交聚合{agg_id}命令{com_id}错误反馈失败：{e}");
                    }
                }
            } else {
                match dispatcher
                    .dispatch(agg_type, agg_id, com_data, agg.clone(), loader)
                    .await
                {
                    Ok((mut na, evt)) => {
                        debug!("聚合{agg_id}命令{com_id}预处理成功");
                        let mut buf = pool.get();
                        match loop {
                            match bincode::encode_into_slice(&evt, buf.as_mut(), BINCODE_CONFIG) {
                                Ok(len) => break Ok(buf.split_to(len).freeze()),
                                Err(EncodeError::UnexpectedEnd) => {
                                    let new_size = buf.capacity() * 2;
                                    buf.reserve(new_size);
                                }
                                Err(e) => break Err(UniError::EncodeError(e)),
                            }
                        } {
                            Ok(bytes) => match stream
                                .write(agg_type, agg_id, com_id, na.revision(), bytes)
                                .await
                            {
                                Ok(()) => {
                                    info!("聚合{agg_id}命令{com_id}写入成功");
                                    na.next();
                                    agg = na;
                                    coms.insert(com_id);
                                }
                                Err(e) => {
                                    warn!("聚合{agg_id}命令{com_id}写入失败：{e}");
                                    let evt_data = Bytes::from(e.to_string());
                                    match stream
                                        .respond(agg_type, agg_id, com_id, e.response(), evt_data)
                                        .await
                                    {
                                        Ok(()) => {
                                            info!("聚合{agg_id}命令{com_id}写入失败反馈成功");
                                            coms.insert(com_id);
                                        }
                                        Err(e) => {
                                            warn!("聚合{agg_id}命令{com_id}写入失败反馈失败：{e}");
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                warn!("聚合{agg_id}命令{com_id}事件序列化错误：{e}");
                                let evt_data = Bytes::from(e.to_string());
                                match stream
                                    .respond(agg_type, agg_id, com_id, e.response(), evt_data)
                                    .await
                                {
                                    Ok(()) => {
                                        info!("聚合{agg_id}命令{com_id}事件序列化错误反馈成功");
                                        coms.insert(com_id);
                                    }
                                    Err(e) => {
                                        warn!(
                                            "聚合{agg_id}命令{com_id}事件序列化错误反馈失败：{e}"
                                        );
                                    }
                                }
                            }
                        }
                        pool.put(buf);
                    }
                    Err(e) => {
                        warn!("聚合{agg_id}命令{com_id}预处理错误：{e}");
                        let evt_data = Bytes::from(e.to_string());
                        match stream
                            .respond(agg_type, agg_id, com_id, e.response(), evt_data)
                            .await
                        {
                            Ok(()) => {
                                info!("聚合{agg_id}命令{com_id}预处理错误反馈成功");
                                coms.insert(com_id);
                            }
                            Err(e) => {
                                warn!("聚合{agg_id}命令{com_id}预处理错误反馈失败：{e}");
                            }
                        }
                    }
                }
            }
        }
    }
}

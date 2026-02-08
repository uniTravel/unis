//! # **unis** 聚合器
//!
//!

use crate::{
    Com, EMPTY_BYTES,
    config::SubscribeConfig,
    domain::{Aggregate, CommandEnum, EventEnum, Load, Restore, Stream},
    errors::UniError,
    response::UniResponse,
};
use ahash::{AHashMap, AHashSet};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
    ser::allocator::Arena,
};
use std::{marker::PhantomData, sync::Arc};
use tokio::{
    sync::{
        Notify,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
    time::{self, Duration, Instant, MissedTickBehavior},
};
use tracing::{Span, error, field, info, instrument, warn};
use uuid::Uuid;

impl<F, Fut> Restore for F
where
    F: Fn(&'static str, i64) -> Fut + Send + 'static,
    Fut: Future<Output = Result<AHashMap<Uuid, AHashSet<Uuid>>, UniError>> + Send,
{
    type Fut = Fut;

    #[inline]
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut {
        self(agg_type, latest)
    }
}

impl<E, F, Fut> Load<E> for F
where
    E: EventEnum,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
    F: Fn(&'static str, Uuid) -> Fut + Send + Copy + 'static,
    Fut: Future<Output = Result<Vec<E>, UniError>> + Send,
{
    type Fut = Fut;

    fn load(&self, agg_type: &'static str, agg_id: Uuid) -> Self::Fut {
        self(agg_type, agg_id)
    }
}

/// 聚合器结构
pub struct Aggregator<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    _marker: PhantomData<C>,
}

impl<A, C, E> Aggregator<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Sync + Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    /// 启动聚合器
    #[instrument(name = "launch_aggregator", skip_all, fields(agg_type))]
    pub async fn launch(
        cfg: SubscribeConfig,
        loader: impl Load<E>,
        stream: Arc<impl Stream>,
        restore: impl Restore,
        mut rx: UnboundedReceiver<Com<C>>,
        ready: Arc<Notify>,
    ) {
        let agg_type = A::topic();
        Span::current().record("agg_type", agg_type);
        let latest = cfg.latest;
        let mut caches: AHashMap<Uuid, (UnboundedSender<Com<C>>, Instant)> = AHashMap::new();
        let start = Instant::now();
        let mut interval = time::interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        match restore.restore(agg_type, latest).await {
            Ok(agg_coms) => {
                for (agg_id, coms) in agg_coms {
                    let (agg_tx, agg_rx) = mpsc::unbounded_channel::<Com<C>>();
                    tokio::spawn(Self::process(
                        agg_type,
                        agg_id,
                        loader,
                        Arc::clone(&stream),
                        coms,
                        agg_rx,
                    ));
                    caches.insert(agg_id, (agg_tx, Instant::now()));
                }
            }
            Err(e) => {
                error!("恢复聚合命令操作记录失败：{e}");
                panic!("恢复聚合命令操作记录失败");
            }
        }
        info!("成功恢复最近 {latest} 分钟的聚合命令操作记录");
        info!("聚合器准备就绪");

        ready.notify_one();
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
                data = rx.recv() => match data {
                    Some(com) => {
                        let agg_id = com.agg_id.clone();
                        if let Some((agg_tx, instant)) = caches.get_mut(&agg_id) {
                            *instant = Instant::now();
                            if let Err(e) = agg_tx.send(com) {
                                error!("提交聚合命令失败：{e}");
                            }
                        } else {
                            let (agg_tx, agg_rx) = mpsc::unbounded_channel::<Com<C>>();
                            tokio::spawn(Self::process(
                                agg_type,
                                agg_id,
                                loader,
                                Arc::clone(&stream),
                                AHashSet::new(),
                                agg_rx,
                            ));
                            if let Err(e) = agg_tx.send(com) {
                                error!("提交聚合命令失败：{e}");
                            }
                            caches.insert(agg_id, (agg_tx, Instant::now()));
                        }
                    }
                    None => {
                        info!("发送端已关闭，聚合器稍后将停止工作");
                        break;
                    }
                }
            }
        }
    }

    #[instrument(
        name = "process_aggregate",
        skip(loader, stream, coms, agg_rx),
        fields(com_id)
    )]
    async fn process(
        agg_type: &'static str,
        agg_id: Uuid,
        loader: impl Load<E>,
        stream: Arc<impl Stream>,
        mut coms: AHashSet<Uuid>,
        mut agg_rx: UnboundedReceiver<Com<C>>,
    ) {
        let mut agg = A::new(agg_id);
        let mut arena = Arena::new();

        loop {
            match agg_rx.recv().await {
                Some(Com {
                    agg_id,
                    com_id,
                    com,
                }) => {
                    Span::current().record("com_id", field::display(&com_id));
                    if coms.contains(&com_id) {
                        warn!("重复提交聚合命令");
                        match stream
                            .respond(
                                agg_type,
                                agg_id,
                                com_id,
                                &UniResponse::Duplicate.to_bytes(),
                                EMPTY_BYTES,
                            )
                            .await
                        {
                            Ok(()) => info!("重复提交聚合命令反馈成功"),
                            Err(e) => error!("重复提交聚合命令反馈失败：{e}"),
                        }
                    } else {
                        match com.apply(agg_type, agg_id, agg.clone(), loader).await {
                            Ok((mut na, evt)) => match evt.to_bytes(&mut arena) {
                                Ok(bytes) => match stream
                                    .write(
                                        agg_type,
                                        agg_id,
                                        com_id,
                                        na.revision(),
                                        bytes.as_slice(),
                                    )
                                    .await
                                {
                                    Ok(()) => {
                                        info!("聚合类型事件写入成功");
                                        na.next();
                                        agg = na;
                                        coms.insert(com_id);
                                    }
                                    Err(e) => {
                                        error!("聚合类型事件写入失败：{e}");
                                        match stream
                                            .respond(
                                                agg_type,
                                                agg_id,
                                                com_id,
                                                &e.response().to_bytes(),
                                                e.to_string().as_bytes(),
                                            )
                                            .await
                                        {
                                            Ok(()) => {
                                                info!("聚合类型事件写入失败反馈成功");
                                                coms.insert(com_id);
                                            }
                                            Err(e) => {
                                                error!("聚合类型事件写入失败反馈失败：{e}")
                                            }
                                        }
                                    }
                                },
                                Err(e) => {
                                    error!("聚合类型事件序列化错误：{e}");
                                    match stream
                                        .respond(
                                            agg_type,
                                            agg_id,
                                            com_id,
                                            &UniResponse::CodeError.to_bytes(),
                                            e.to_string().as_bytes(),
                                        )
                                        .await
                                    {
                                        Ok(()) => {
                                            info!("聚合类型事件序列化错误反馈成功");
                                            coms.insert(com_id);
                                        }
                                        Err(e) => {
                                            error!("聚合类型事件序列化错误反馈失败：{e}")
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error!("聚合命令预处理错误：{e}");
                                match stream
                                    .respond(
                                        agg_type,
                                        agg_id,
                                        com_id,
                                        &e.response().to_bytes(),
                                        e.to_string().as_bytes(),
                                    )
                                    .await
                                {
                                    Ok(()) => {
                                        info!("聚合命令预处理错误反馈成功");
                                        coms.insert(com_id);
                                    }
                                    Err(e) => error!("聚合命令预处理错误反馈失败：{e}"),
                                }
                            }
                        }
                    }
                }
                None => break,
            }
        }
    }
}

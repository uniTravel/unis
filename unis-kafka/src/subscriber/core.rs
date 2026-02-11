use super::{SUBSCRIBER_CONFIG, reader, stream::Writer};
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Headers},
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::{
    Notify,
    mpsc::{self},
};
use tracing::{Span, debug, error, info, instrument};
use unis::{
    Com,
    aggregator::Aggregator,
    domain::{Aggregate, CommandEnum, EventEnum},
    errors::UniError,
};
use uuid::Uuid;

pub struct Subscriber<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    _marker: PhantomData<C>,
}

impl<A, C, E> Subscriber<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Sync + Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[instrument(name = "launch_subscriber", skip_all, fields(agg_type))]
    pub async fn launch() -> Result<(), String> {
        let context = super::app::app().await;
        let agg_type = A::topic();
        Span::current().record("agg_type", agg_type);
        let cfg_name = agg_type.rsplit(".").next().ok_or("获取聚合名称失败")?;
        let settings = SUBSCRIBER_CONFIG
            .cc
            .get(cfg_name)
            .ok_or("获取订阅者消费配置失败")?;
        let cfg = SUBSCRIBER_CONFIG.subscriber.get(cfg_name);
        let topic = A::topic_com();
        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
        config.set("group.id", topic);
        let cc: Arc<StreamConsumer> = Arc::new(
            config
                .create()
                .map_err(|e| format!("订阅者消费创建失败：{e}"))?,
        );
        cc.subscribe(&[topic])
            .map_err(|e| format!("订阅聚合命令流失败：{e}"))?;
        info!("成功订阅 {topic} 聚合命令流");

        let (tx, rx) = mpsc::unbounded_channel::<Com<C>>();
        let stream = Arc::new(Writer::new(&cfg, context.topic_tx()));
        context
            .spawn(move |ready| {
                Aggregator::<A, C, E>::launch(cfg, reader::load, stream, reader::restore, rx, ready)
            })
            .await;
        context
            .spawn_notify(move |ready, notify| Self::consume(agg_type, cc, tx, ready, notify))
            .await;
        Ok(())
    }

    #[instrument(name = "receive_command", skip(cc, tx, ready, notify))]
    async fn consume(
        agg_type: &'static str,
        cc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Com<C>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let notified = notify.notified();
        tokio::pin!(notified);
        ready.notify_one();
        loop {
            tokio::select! {
                biased;
                _ = &mut notified => {
                    info!("收到关闭信号，开始优雅退出");
                    break;
                }
                data = cc.recv() => match data {
                    Ok(msg) => {
                        match Self::process_message(&msg) {
                            Ok(com) => {
                                if let Err(e) = tx.send(com) {
                                    error!("提交聚合命令失败：{e}");
                                }
                            }
                            Err(e) => error!("{e}"),
                        }
                    }
                    Err(e) => error!("消息错误：{e}"),
                }
            }
        }
    }

    fn process_message(msg: &BorrowedMessage<'_>) -> Result<Com<C>, UniError> {
        let key = msg.key().ok_or("消息键不存在")?;
        let agg_id = Uuid::from_slice(key).map_err(|e| UniError::MsgError(e.to_string()))?;
        debug!("提取聚合Id：{agg_id}");

        let id = msg
            .headers()
            .ok_or("消息头不存在")?
            .iter()
            .find(|h| h.key == "com_id")
            .ok_or("键为'com_id'的消息头不存在")?
            .value
            .ok_or("键'com_id'对应的值为空")?;
        let com_id = Uuid::from_slice(id).map_err(|e| UniError::MsgError(e.to_string()))?;
        debug!("提取命令Id：{com_id}");

        let com_data = msg.payload().ok_or("空消息体")?;
        Ok(Com {
            agg_id,
            com_id,
            com: C::from_bytes(com_data)?,
        })
    }
}

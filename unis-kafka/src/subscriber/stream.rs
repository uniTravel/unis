use super::{SUBSCRIBER_CONFIG, TopicTask};
use crate::BINCODE_HEADER;
use rdkafka::{
    ClientConfig,
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
};
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc;
use tracing::{debug, error, instrument};
use unis::{Response, config::SubscribeConfig, domain, errors::UniError};
use uuid::Uuid;

static SHARED_TP: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
    Arc::new(config.create().expect("共享的聚合类型生产者创建失败"))
});

static TP_CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    for (key, value) in &SUBSCRIBER_CONFIG.tp {
        config.set(key, value);
    }
    config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
    config
});

pub(crate) struct Writer {
    topic_tx: mpsc::UnboundedSender<TopicTask>,
    producer: Arc<FutureProducer>,
}

impl Writer {
    pub fn new(cfg: &SubscribeConfig, topic_tx: mpsc::UnboundedSender<TopicTask>) -> Self {
        let producer = match cfg.hotspot {
            true => Arc::new(TP_CONFIG.create().expect("聚合类型生产者创建失败")),
            false => Arc::clone(&SHARED_TP),
        };
        Self { topic_tx, producer }
    }
}

impl domain::Stream for Writer {
    #[instrument(name = "stream_write", level = "debug", skip(self, revision, evt_data))]
    async fn write(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: &[u8],
    ) -> Result<(), UniError> {
        if revision == u64::MAX {
            debug!("创建聚合主题");
            if let Err(e) = self.topic_tx.send(TopicTask { agg_type, agg_id }) {
                error!(agg_type, %agg_id, "发送聚合主题失败：{e}");
            }
        }

        let mut buf = [0u8; 4];
        bincode::encode_into_slice(Response::Success, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data)
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "response",
                        value: Some(&buf),
                    }),
            );

        self.producer
            .send(record, SUBSCRIBER_CONFIG.timeout)
            .await
            .map_err(|(e, _)| UniError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("生成的事件写到分区 {partition} 偏移 {offset}")
                },
            )
    }

    #[instrument(name = "stream_respond", level = "debug", skip(self, res, evt_data))]
    async fn respond(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        res: Response,
        evt_data: &[u8],
    ) -> Result<(), UniError> {
        let mut buf = [0u8; 4];
        bincode::encode_into_slice(res, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data)
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "response",
                        value: Some(&buf),
                    }),
            );

        self.producer
            .send(record, SUBSCRIBER_CONFIG.timeout)
            .await
            .map_err(|(e, _)| UniError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("生成的反馈事件写到分区 {partition} 偏移 {offset}")
                },
            )
    }
}

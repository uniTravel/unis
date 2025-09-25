use crate::{
    BINCODE_HEADER,
    {
        subscriber::SUBSCRIBER_CONFIG,
        topic::{TOPIC_TX, TopicTask},
    },
};
use bincode::encode_into_slice;
use bytes::Bytes;
use rdkafka::{
    ClientConfig,
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
};
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use unis::{Res, config::AggConfig, domain::Stream, errors::DomainError};
use uuid::Uuid;

static SHARED: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
    Arc::new(config.create().expect("共享的聚合类型生产者创建失败"))
});

static TP_CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    let cfg = &SUBSCRIBER_CONFIG.tp;
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
    for (key, value) in cfg {
        config.set(key, value);
    }
    config
});

pub(crate) struct KafkaStream {
    topic_tx: mpsc::UnboundedSender<TopicTask>,
    producer: Arc<FutureProducer>,
}

impl KafkaStream {
    pub fn new(cfg: &AggConfig) -> Self {
        LazyLock::force(&TOPIC_TX);
        let producer = match cfg.hotspot {
            true => Arc::new(TP_CONFIG.create().expect("聚合类型生产者创建失败")),
            false => SHARED.clone(),
        };
        Self {
            topic_tx: TOPIC_TX.clone(),
            producer,
        }
    }
}

impl Stream for KafkaStream {
    async fn write(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: Bytes,
    ) -> Result<(), DomainError> {
        if revision == u64::MAX {
            if let Err(e) = self.topic_tx.send(TopicTask { agg_type, agg_id }) {
                warn!("发送聚合主题{agg_type}-{agg_id}失败：{e}");
            }
        }

        let mut buf = [0u8; 1];
        encode_into_slice(Res::Success, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data.as_ref())
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "res",
                        value: Some(&buf),
                    }),
            );

        self.producer
            .send(record, SUBSCRIBER_CONFIG.timeout)
            .await
            .map_err(|(e, _)| DomainError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("聚合{agg_id}命令{com_id}的事件写入分区{partition}偏移{offset}")
                },
            )
    }

    async fn respond(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        res: Res,
        evt_data: Bytes,
    ) -> Result<(), DomainError> {
        let mut buf = [0u8; 1];
        encode_into_slice(res, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data.as_ref())
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "res",
                        value: Some(&buf),
                    }),
            );

        self.producer
            .send(record, SUBSCRIBER_CONFIG.timeout)
            .await
            .map_err(|(e, _)| DomainError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("聚合{agg_id}命令{com_id}的事件写入分区{partition}偏移{offset}")
                },
            )
    }
}

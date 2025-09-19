//! 向Kafka写入数据

use crate::{
    BINCODE_HEADER, aggregator::Res, errors::DomainError, kafka::subscriber::SUBSCRIBER_CONFIG,
};
use bincode::encode_into_slice;
use bytes::Bytes;
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
};
use std::{sync::LazyLock, time::Duration};
use tracing::debug;
use uuid::Uuid;

static TP: LazyLock<FutureProducer> = LazyLock::new(|| {
    let cfg = &SUBSCRIBER_CONFIG.tp;
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
    for (key, value) in cfg {
        config.set(key, value);
    }
    config.create().expect("聚合类型生产者创建失败")
});

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    ClientConfig::new()
        .set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap)
        .create()
        .expect("恢复命令操作记录消费者创建失败")
});

///
pub async fn write(
    agg_type: &'static str,
    agg_id: Uuid,
    com_id: Uuid,
    revision: u64,
    res: Res,
    evt_data: Bytes,
) -> Result<(), DomainError> {
    match res {
        Res::Success if revision == u64::MAX => {
            let mut topic = String::with_capacity(agg_type.len() + 37);
            topic.push_str(agg_type);
            topic.push_str("-");
            topic.push_str(&agg_id.to_string());
            tokio::spawn(async move {
                let topic =
                    NewTopic::new(&topic, 1, TopicReplication::Fixed(2)).set("retention.ms", "-1");
                let opts = AdminOptions::new()
                    .operation_timeout(Some(Duration::from_secs(30)))
                    .request_timeout(Some(Duration::from_secs(45)));
                let _ = ADMIN.create_topics(vec![&topic], &opts).await;
            });
        }
        _ => (),
    }

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

    TP.send(record, SUBSCRIBER_CONFIG.timeout)
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

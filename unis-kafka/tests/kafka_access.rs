mod common;

use crate::common::{ADMIN, CFG, config};
use futures::StreamExt;
use rdkafka::{
    Message,
    admin::{AdminOptions, NewTopic, TopicReplication},
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
};
use std::{collections::HashMap, sync::LazyLock};
use tokio::time::Duration;
use uuid::Uuid;

static PRODUCER: LazyLock<FutureProducer> =
    LazyLock::new(|| config(HashMap::new()).create().unwrap());

static CONSUMER: LazyLock<StreamConsumer> = LazyLock::new(|| {
    let mut settings: HashMap<&str, &str> = HashMap::new();
    settings.insert("group.id", "group");
    settings.insert("auto.offset.reset", "earliest");
    config(settings).create().unwrap()
});

#[test]
fn get_config() {
    let bootstrap: String = CFG.get("bootstrap").unwrap();
    assert_eq!(bootstrap, "localhost:30001,localhost:30002,localhost:30003");
}

#[tokio::test]
#[ignore = "仅用于测试Kafka集群连接与基础操作"]
async fn admin_create_topic() {
    let topic = &Uuid::new_v4().to_string();
    let opts = AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(30)))
        .request_timeout(Some(Duration::from_secs(45)));
    let topic = NewTopic::new(topic, 1, TopicReplication::Fixed(3));
    assert!(ADMIN.create_topics(&[topic], &opts).await.is_ok());
}

#[tokio::test]
#[ignore = "仅用于测试Kafka集群连接与基础操作"]
async fn produce_to_exist_topic() {
    let topic = &Uuid::new_v4().to_string();
    let opts = AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(30)))
        .request_timeout(Some(Duration::from_secs(45)));
    let record = FutureRecord::to(topic).key("key").payload("message");
    let topic = NewTopic::new(topic, 1, TopicReplication::Fixed(3));
    assert!(ADMIN.create_topics(&[topic], &opts).await.is_ok());
    assert!(PRODUCER.send(record, Duration::from_secs(15)).await.is_ok());
}

#[tokio::test]
#[ignore = "仅用于测试Kafka集群连接与基础操作"]
async fn produce_to_nonexist_topic() {
    let topic = &Uuid::new_v4().to_string();
    let record = FutureRecord::to(topic).key("key").payload("message");
    assert!(PRODUCER.send(record, Duration::from_secs(15)).await.is_ok());
}

#[tokio::test]
#[ignore = "仅用于测试Kafka集群连接与基础操作"]
async fn consume_exist_topic() {
    let topic = &Uuid::new_v4().to_string();
    let record = FutureRecord::to(topic).key("com_id").payload("command");
    assert!(PRODUCER.send(record, Duration::from_secs(15)).await.is_ok());
    CONSUMER.subscribe(&[topic]).unwrap();
    let stream = CONSUMER.stream();
    tokio::pin!(stream);
    let msg = stream.next().await.unwrap().unwrap();
    assert_eq!(str::from_utf8(msg.key().unwrap()).unwrap(), "com_id");
    assert_eq!(str::from_utf8(msg.payload().unwrap()).unwrap(), "command");
}

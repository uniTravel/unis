use super::*;
use crate::{stream::Writer, subscriber::SUBSCRIBER_CONFIG};
use bytes::Bytes;

static STREAM: LazyLock<Writer> = LazyLock::new(|| {
    let agg_type = note::Note::topic();
    let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
    let cfg = SUBSCRIBER_CONFIG.subscriber.get(cfg_name);
    Writer::new(&cfg)
});

#[tokio::test]
async fn write_to_stream_with_agg_topic_creation() {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let evt_data = Bytes::new();
    let result = STREAM
        .write(agg_type, agg_id, com_id, u64::MAX, evt_data)
        .await;
    sleep(Duration::from_millis(100)).await;

    let name = &agg_topic(agg_type, agg_id);
    assert!(is_topic_exist(name));
    assert!(result.is_ok());
}

#[tokio::test]
async fn write_to_stream_without_agg_topic_creation() {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let evt_data = Bytes::new();
    let result = STREAM.write(agg_type, agg_id, com_id, 0, evt_data).await;
    sleep(Duration::from_millis(100)).await;

    let name = &agg_topic(agg_type, agg_id);
    assert!(!is_topic_exist(name));
    assert!(result.is_ok());
}

#[tokio::test]
async fn respond_to_stream() {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let evt_data = Bytes::new();
    let result = STREAM
        .respond(agg_type, agg_id, com_id, Response::Duplicate, evt_data)
        .await;
    sleep(Duration::from_millis(100)).await;

    let name = &agg_topic(agg_type, agg_id);
    assert!(!is_topic_exist(name));
    assert!(result.is_ok());
}

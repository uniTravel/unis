use super::{ADMIN, OPTS, ctx, internal_setup, is_agg_topic_exist, stream};
use crate::subscriber::Topic;
use rdkafka::admin::{NewTopic, TopicReplication};
use rstest::{fixture, rstest};
use unis::{UniResponse, app::Context, subscriber::Stream};
use uuid::Uuid;

#[fixture]
async fn init(#[future(awt)] _internal_setup: ()) {
    let topic = domain::Account::topic();
    let topic_com = domain::Account::topic_com();
    let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
    let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&vec![agg, com], &OPTS).await;
}

#[rstest]
#[tokio::test]
async fn write_with_agg_topic(#[future(awt)] _init: (), ctx: &'static Context) {
    let stream = stream().await;
    let topic = domain::Account::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream.write(topic, agg_id, com_id, 0, &[]).await;

    assert!(result.is_ok());
    assert!(is_agg_topic_exist(topic, agg_id).await);
    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn write_without_agg_topic(#[future(awt)] _init: (), ctx: &'static Context) {
    let stream = stream().await;
    let topic = domain::Account::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream.write(topic, agg_id, com_id, 1, &[]).await;

    assert!(result.is_ok());
    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn respond_to_stream(#[future(awt)] _init: (), ctx: &'static Context) {
    let stream = stream().await;
    let topic = domain::Account::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream
        .respond(
            topic,
            agg_id,
            com_id,
            &UniResponse::Duplicate.to_bytes(),
            &[],
        )
        .await;

    assert!(result.is_ok());
    ctx.teardown().await;
}

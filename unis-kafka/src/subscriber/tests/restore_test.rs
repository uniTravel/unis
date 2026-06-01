use super::{ADMIN, OPTS, ctx, internal_setup, stream};
use crate::subscriber::{Topic, reader};
use rdkafka::admin::{NewTopic, TopicReplication};
use rstest::{fixture, rstest};
use unis::{app::Context, subscriber::Stream};
use uuid::Uuid;

#[fixture]
async fn init(#[future(awt)] _internal_setup: ()) {
    let topic = domain::Account::topic();
    let topic_com = domain::Account::topic_com();
    let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
    let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
    let name = NewTopic::new("note.Restore", 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&vec![agg, com, name], &OPTS).await;
}

#[rstest]
#[tokio::test]
async fn restore_without_coms(#[future(awt)] _init: ()) {
    let topic = "note.Restore";

    let agg_coms = reader::restore(topic, 1).await.unwrap();

    assert_eq!(agg_coms.len(), 0);
}

#[rstest]
#[tokio::test]
async fn restore_with_coms(#[future(awt)] _init: (), ctx: &'static Context) {
    let stream = stream().await;
    let topic = domain::Account::topic();
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let com_id = com_id.as_bytes();
    let span_id = Uuid::new_v4();
    let span_id = span_id.as_bytes()[..8].try_into().unwrap();
    let result = stream.write(topic, agg_id, com_id, span_id, 3, &[]).await;
    assert!(result.is_ok());

    let agg_coms = reader::restore(topic, 1).await.unwrap();
    let (revision, coms) = agg_coms.get(&agg_id).unwrap();

    assert!(agg_coms.len() >= 1);
    assert!(*revision == 3);
    assert!(coms.len() == 1);
    ctx.teardown().await;
}

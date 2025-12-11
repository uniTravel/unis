use super::*;

#[fixture]
async fn init(#[future(awt)] _internal_setup: ()) {
    let topic = note::Note::topic();
    let topic_com = note::Note::topic_com();
    let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
    let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&vec![agg, com], &OPTS).await;
}

#[rstest]
#[tokio::test]
async fn write_with_agg_topic(#[future(awt)] _init: (), #[future(awt)] context: Arc<App>) {
    let stream = stream(&context);
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream.write(agg_type, agg_id, com_id, u64::MAX, &[]).await;

    assert!(result.is_ok());
    assert!(is_agg_topic_exist(agg_type, agg_id).await);
    context.teardown().await;
}

#[rstest]
#[tokio::test]
async fn write_without_agg_topic(#[future(awt)] _init: (), #[future(awt)] context: Arc<App>) {
    let stream = stream(&context);
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream.write(agg_type, agg_id, com_id, 0, &[]).await;

    assert!(result.is_ok());
    context.teardown().await;
}

#[rstest]
#[tokio::test]
async fn respond_to_stream(#[future(awt)] _init: (), #[future(awt)] context: Arc<App>) {
    let stream = stream(&context);
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream
        .respond(agg_type, agg_id, com_id, Response::Duplicate, &[])
        .await;

    assert!(result.is_ok());
    context.teardown().await;
}

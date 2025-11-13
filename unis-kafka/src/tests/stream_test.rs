use super::*;

#[fixture]
async fn stream_context() {
    LazyLock::force(&INTERNAL_SETUP);
    static ONCE: OnceCell<()> = OnceCell::const_new();
    ONCE.get_or_init(|| async {
        info!("一次性初始化 Stream 测试上下文");
        let topic = note::Note::topic();
        let topic_com = note::Note::topic_com();
        let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
        let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
        let _ = ADMIN.create_topics(&[agg, com], &OPTS).await;
    })
    .await;
}

#[rstest]
#[tokio::test]
async fn write_to_stream_with_agg_topic_creation(
    #[from(stream_context)]
    #[future(awt)]
    _init: (),
) {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = STREAM.write(agg_type, agg_id, com_id, u64::MAX, &[]).await;

    assert!(result.is_ok());
    assert!(is_agg_topic_exist(agg_type, agg_id).await);
}

#[rstest]
#[tokio::test]
async fn write_to_stream_without_agg_topic_creation(
    #[from(stream_context)]
    #[future(awt)]
    _init: (),
) {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = STREAM.write(agg_type, agg_id, com_id, 0, &[]).await;

    assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn respond_to_stream(
    #[from(stream_context)]
    #[future(awt)]
    _init: (),
) {
    let agg_type = note::Note::topic();

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = STREAM
        .respond(agg_type, agg_id, com_id, Response::Duplicate, &[])
        .await;

    assert!(result.is_ok());
}

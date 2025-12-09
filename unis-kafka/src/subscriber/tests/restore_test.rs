use super::*;

#[fixture]
async fn init(#[future(awt)] _internal_setup: ()) {
    let topic = note::Note::topic();
    let topic_com = note::Note::topic_com();
    let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
    let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
    let name = NewTopic::new("note.Restore", 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&vec![agg, com, name], &OPTS).await;
}

#[rstest]
#[tokio::test]
async fn restore_without_coms(#[future(awt)] _init: ()) {
    let topic = "note.Restore";

    let agg_coms = restore(topic, 1).await.unwrap();

    assert_eq!(agg_coms.len(), 0);
}

#[rstest]
#[tokio::test]
async fn restore_with_coms(#[future(awt)] _init: (), #[future(awt)] context: (Arc<App>, Writer)) {
    let (app, stream) = context;
    let agg_type = note::Note::topic();
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let result = stream.write(agg_type, agg_id, com_id, 0, &[]).await;
    assert!(result.is_ok());

    let agg_coms = restore(agg_type, 1).await.unwrap();

    assert!(agg_coms.len() >= 1);
    app.shutdown().await;
}

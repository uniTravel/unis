use super::*;
use crate::reader::restore;

#[fixture]
async fn reader_context() {
    LazyLock::force(&INTERNAL_SETUP);
    static ONCE: OnceCell<()> = OnceCell::const_new();
    ONCE.get_or_init(|| async {
        info!("一次性初始化 Reader 测试上下文");
        let topic = note::Note::topic();
        let topic_com = note::Note::topic_com();
        let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
        let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
        let name = NewTopic::new("note.Restore", 3, TopicReplication::Fixed(3));
        let _ = ADMIN.create_topics(&[agg, com, name], &OPTS).await;
    })
    .await;
}

#[rstest]
#[tokio::test]
async fn restore_without_coms(
    #[from(reader_context)]
    #[future(awt)]
    _init: (),
) {
    let topic = "note.Restore";

    let agg_coms = restore(topic, 1).await.unwrap();

    assert_eq!(agg_coms.len(), 0);
}

#[rstest]
#[tokio::test]
async fn restore_with_coms(
    #[from(reader_context)]
    #[future(awt)]
    _init: (),
) {
    let agg_type = note::Note::topic();
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let evt_data = Bytes::new();
    let result = STREAM.write(agg_type, agg_id, com_id, 0, evt_data).await;
    assert!(result.is_ok());

    let agg_coms = restore(agg_type, 1).await.unwrap();

    assert!(agg_coms.len() >= 1);
}

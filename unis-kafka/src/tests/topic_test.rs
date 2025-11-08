use super::*;
use crate::topic::{TOPIC_TX, TopicTask};

#[fixture]
async fn topic_context() {
    LazyLock::force(&INTERNAL_SETUP);
    static ONCE: OnceCell<()> = OnceCell::const_new();
    ONCE.get_or_init(|| async {
        info!("一次性初始化 Topic 测试上下文");
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
async fn check_topic_exist(
    #[from(topic_context)]
    #[future(awt)]
    _init: (),
) {
    let topic = note::Note::topic();
    let topic_com = note::Note::topic_com();

    assert!(is_topic_exist(&topic).await);
    assert!(is_topic_exist(&topic_com).await);
}

#[rstest]
#[tokio::test]
async fn create_agg_topic(
    #[from(topic_context)]
    #[future(awt)]
    _init: (),
) {
    let tx = TOPIC_TX.clone();
    let agg_type = note::Note::topic();
    let agg_id = Uuid::new_v4();

    let _ = tx.send(TopicTask { agg_type, agg_id });

    assert!(is_agg_topic_exist(agg_type, agg_id).await);
}

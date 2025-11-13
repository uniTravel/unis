use super::*;
use crate::{
    reader::{load, restore},
    topic::{TOPIC_TX, TopicTask},
};
use domain::note::{NoteChanged, NoteCreated, NoteEvent};
use unis::{BINCODE_CONFIG, pool::BufferPool};

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
    let result = STREAM.write(agg_type, agg_id, com_id, 0, &[]).await;
    assert!(result.is_ok());

    let agg_coms = restore(agg_type, 1).await.unwrap();

    assert!(agg_coms.len() >= 1);
}

#[rstest]
#[tokio::test]
async fn load_agg(
    #[from(reader_context)]
    #[future(awt)]
    _init: (),
) {
    let pool = BufferPool::new(1024, 8);
    let tx = TOPIC_TX.clone();
    let agg_type = note::Note::topic();
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let _ = tx.send(TopicTask { agg_type, agg_id });
    let note_created = NoteEvent::Created(NoteCreated {
        title: "title".to_string(),
        content: "content".to_string(),
        grade: 1,
    });
    let mut buf = pool.get();
    let len = bincode::encode_into_slice(&note_created, &mut buf, BINCODE_CONFIG).unwrap();
    let create = STREAM
        .write_to_agg(agg_type, agg_id, com_id, &buf[..len])
        .await;
    assert!(create.is_ok());
    pool.put(buf);
    let mut buf = pool.get();
    let com_id = Uuid::new_v4();
    let note_changed = NoteEvent::Changed(NoteChanged {
        content: "content1".to_string(),
    });
    let len = bincode::encode_into_slice(&note_changed, &mut buf, BINCODE_CONFIG).unwrap();
    let change = STREAM
        .write_to_agg(agg_type, agg_id, com_id, &buf[..len])
        .await;
    assert!(change.is_ok());
    pool.put(buf);

    let result = load(agg_type, agg_id).await;

    match result {
        Ok(ds) => {
            for evt_data in ds {
                let (evt, _): (NoteEvent, _) =
                    ::bincode::decode_from_slice(&evt_data, BINCODE_CONFIG).unwrap();
                match evt {
                    NoteEvent::Created(created) => assert_eq!(
                        (created.title, created.content, created.grade),
                        ("title".to_string(), "content".to_string(), 1)
                    ),
                    NoteEvent::Changed(changed) => {
                        assert_eq!(changed.content, "content1".to_string())
                    }
                }
            }
        }
        Err(e) => {
            warn!("加载聚合出错：{e}");
            assert!(false);
        }
    }
}

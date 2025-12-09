mod common;

// use crate::common::*;
// use domain::note::{self, CreateNote, Note, NoteCommand};
// use rdkafka::admin::{NewTopic, TopicReplication};
// use rstest::*;
// use std::sync::LazyLock;
// use tokio::{
//     sync::OnceCell,
//     time::{Duration, sleep},
// };
// use tracing::info;
// use unis::{
//     Response,
//     domain::{Aggregate, Request},
// };
// use unis_kafka::sender::Sender;
// use uuid::Uuid;

// #[fixture]
// async fn sender() -> &'static Sender<Note, NoteCommand> {
//     LazyLock::force(&EXTERNAL_SETUP);
//     static ONCE: OnceCell<Sender<Note, NoteCommand>> = OnceCell::const_new();
//     ONCE.get_or_init(|| async {
//         info!("一次性初始化测试上下文");
//         let topic = note::Note::topic();
//         let topic_com = note::Note::topic_com();
//         let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
//         let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
//         match ADMIN.create_topics(&[agg, com], &OPTS).await {
//             Ok(_) => {
//                 // tokio::spawn(Subscriber::launch(note::dispatcher, load));
//                 sleep(Duration::from_millis(1000)).await;
//                 Sender::new().await
//             }
//             Err(e) => panic!("创建聚合类型、命令主题失败：{e}"),
//         }
//     })
//     .await
// }

// #[rstest]
// #[tokio::test]
// async fn create_note(#[future(awt)] sender: &'static Sender<Note, NoteCommand>) {
//     let note = CreateNote {
//         title: "title".to_string(),
//         content: "content".to_string(),
//     };
//     let com = NoteCommand::Create(note);
//     let agg_id = Uuid::new_v4();
//     let com_id = Uuid::new_v4();
//     let response = sender.send(agg_id, com_id, com).await;
//     assert_eq!(response, Response::Success);
//     sleep(Duration::from_millis(500)).await;
// }

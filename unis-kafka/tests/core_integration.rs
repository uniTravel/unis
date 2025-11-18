mod common;

use crate::common::*;
use domain::note::{self, CreateNote, Note, NoteCommand};
use rdkafka::admin::{AdminOptions, NewTopic, TopicReplication};
use rstest::*;
use std::sync::LazyLock;
use tokio::{sync::OnceCell, time::Duration};
use tracing::info;
use unis::{
    Response,
    domain::{Aggregate, Request},
};
use unis_kafka::{reader::load, sender::Sender, subscriber::Subscriber};
use uuid::Uuid;

#[fixture]
async fn reader_context() {
    LazyLock::force(&EXTERNAL_SETUP);
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

static SENDER: OnceCell<Sender<Note, NoteCommand>> = OnceCell::const_new();

async fn get_sender() -> &'static Sender<Note, NoteCommand> {
    SENDER
        .get_or_init(|| async {
            let topic = note::Note::topic();
            let topic_com = note::Note::topic_com();
            let opts = AdminOptions::new()
                .operation_timeout(Some(Duration::from_secs(30)))
                .request_timeout(Some(Duration::from_secs(45)));
            let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
            let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
            match ADMIN.create_topics(&[agg, com], &opts).await {
                Ok(_) => {
                    Subscriber::launch(note::dispatcher, load).await;
                    Sender::new().await
                }
                Err(e) => panic!("{}", e),
            }
        })
        .await
}

#[tokio::test]
async fn create_note() {
    let sender = get_sender().await;
    let note = CreateNote {
        title: "title".to_string(),
        content: "content".to_string(),
    };
    let com = NoteCommand::Create(note);
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let response = sender.send(agg_id, com_id, com).await;
    assert_eq!(response, Response::Success);
}

// #[tokio::test]
// async fn launch_subscriber() {
//     Subscriber::launch(note::dispatcher, load).await;
// }

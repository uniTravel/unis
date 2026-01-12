mod common;

use crate::common::*;
use std::sync::Mutex;
use unis_kafka::projector;

#[fixture]
async fn init(#[future(awt)] _external_setup: ()) {
    let topic = note::Note::topic();
    let topic_com = note::Note::topic_com();
    let agg = NewTopic::new(topic, 3, TopicReplication::Fixed(3));
    let com = NewTopic::new(topic_com, 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&vec![agg, com], &OPTS).await;
}

#[rstest]
#[tokio::test]
async fn create_note(
    #[future(awt)] _init: (),
    #[future(awt)] ctx_subscriber: Arc<subscriber::app::App>,
    #[future(awt)] ctx_sender: Arc<sender::app::App>,
    #[future(awt)] ctx_projector: Arc<Mutex<projector::app::App>>,
) {
    std::thread::spawn(move || {
        let mut guard = ctx_projector.lock().unwrap();
        guard.subscribe::<note::Note>();
        guard.launch();
    });
    ctx_subscriber
        .setup(note::dispatcher, subscriber::load)
        .await;
    let sender = ctx_sender.setup().await;

    let note = CreateNote {
        title: "title".to_string(),
        content: "content".to_string(),
    };
    let com = NoteCommand::Create(note);
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let response = sender.send(agg_id, com_id, com).await;

    assert_eq!(response, Response::Success);
    ctx_sender.teardown().await;
    ctx_subscriber.teardown().await;
    projector::App::teardown();
}

mod common;
use common::{
    note::{self, CreateNote, NoteCommand},
    stream,
};
use tokio::time::Duration;
use unis::{
    BINCODE_CONFIG,
    aggregator::{Aggregator, loader},
};
use uuid::Uuid;

use crate::common::note::ChangeNote;

#[tokio::test]
async fn test1() {
    let svc = Aggregator::new(
        100,
        Duration::from_secs(15),
        note::dispatcher::<stream::UniStream>,
        loader::<note::Note, note::Replayer, stream::UniStream>,
        note::Replayer,
        stream::UniStream,
    )
    .await;

    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();
    let com = NoteCommand::Create(CreateNote {
        title: String::from("title"),
        content: String::from("content"),
    });
    let com_data = bincode::encode_to_vec(&com, BINCODE_CONFIG).unwrap();
    let result = svc.commit(agg_id, com_id, com_data).await;
    assert!(result.is_ok());

    let com = NoteCommand::Change(ChangeNote {
        content: String::from("content1"),
    });
    let com_data = bincode::encode_to_vec(&com, BINCODE_CONFIG).unwrap();
    let result = svc.commit(agg_id, com_id, com_data).await;
    assert!(result.is_ok());
}

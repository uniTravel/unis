mod common;
use common::{
    note::{self, CreateNote, NoteCommand},
    stream,
};
use unis::{
    BINCODE_CONFIG,
    aggregator::{Aggregator, loader},
    config::UniConfig,
};
use uuid::Uuid;

use crate::common::note::ChangeNote;

#[tokio::test]
async fn test1() {
    UniConfig::initialize().unwrap();
    // let cfg = UniConfig::get().unwrap().aggregates.get("note");
    let cfg = UniConfig::get().unwrap().aggregates.default();
    let svc = Aggregator::new(
        cfg,
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

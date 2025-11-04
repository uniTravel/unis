use super::*;
use crate::reader::restore;

#[tokio::test]
async fn reader_restore_without_coms() {
    let topic = note::Note::topic();
    // ADMIN.delete_topics(&[topic], opts);
    let agg_coms = restore(topic, 1).await.unwrap();
    assert_eq!(agg_coms.len(), 0);
}

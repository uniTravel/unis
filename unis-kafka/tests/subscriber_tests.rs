mod common;

// use aggregate::note::dispatcher;
// use unis_kafka::{reader::load, subscriber::Subscriber};
use tracing::{debug, info, warn};

#[tokio::test]
async fn test1() {
    // Subscriber::launch(dispatcher, load).await;
    common::init();
    debug!("debug");
    info!("info");
    warn!("warn");
}

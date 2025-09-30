use aggregate::note::dispatcher;
use unis_kafka::{reader::load, subscriber::Subscriber};

#[tokio::test]
async fn test1() {
    Subscriber::launch(dispatcher, load).await;
}

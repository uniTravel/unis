use aggregate::note::dispatcher;
use unis_kafka::{reader::load, subscriber::Subscriber};

#[test]
fn test1() {
    let sub = Subscriber::new(dispatcher, load);
}

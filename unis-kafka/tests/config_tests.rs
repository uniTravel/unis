use unis::domain::Config;
use unis_kafka::config::SubscriberConfig;

#[test]
fn subscriber_test() {
    SubscriberConfig::initialize();
    let cfg = SubscriberConfig::get().unwrap();
    let agg = cfg.aggregates.get("note");
    assert_eq!(agg.capacity, 100);
    assert_eq!(agg.interval, 120);
    assert_eq!(agg.low, 200);
    assert_eq!(agg.high, 20000);
    assert_eq!(agg.retain, 7200);
    let cc = cfg.cc.get("note").unwrap();
    assert_eq!(cc.get("bootstrap.servers").unwrap(), "localhost:9092");
    assert_eq!(cc.get("group.id").unwrap(), "cc-group");
}

use domain::{account::Account, transaction::Transaction};
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
use tokio::time::Duration;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::domain::Aggregate;
use unis_utils::kube::{HelmRelease, KubeCluster};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let namespace = "bank";
    let cluster = KubeCluster::new(namespace);
    let kafka = HelmRelease::new(
        "kafka",
        std::env::home_dir()
            .unwrap()
            .join(".cache/helm/repository/kafka-0.1.0.tgz"),
        namespace,
    );
    cluster.create_namespace().unwrap();
    kafka.install(None).await.unwrap();

    let bootstrap = "localhost:30001,localhost:30002,localhost:30003";
    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .expect("管理客户端创建失败");
    let opts = AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)));

    let mut topics = Vec::new();
    topic::<Account>(&mut topics);
    topic::<Transaction>(&mut topics);
    let _ = admin.create_topics(&topics, &opts).await;
}

fn topic<A: Aggregate>(topics: &mut Vec<NewTopic<'_>>) {
    topics.push(NewTopic::new(A::topic(), 3, TopicReplication::Fixed(3)));
    topics.push(NewTopic::new(A::topic_com(), 3, TopicReplication::Fixed(3)));
}

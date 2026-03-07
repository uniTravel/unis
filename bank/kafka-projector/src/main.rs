use domain::{account, transaction};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::projector::{self, Aggregate};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let ctx = projector::context().await;
    projector::launch(
        ctx,
        vec![account::Account::topic(), transaction::Transaction::topic()],
    )
    .await;

    ctx.all_done().await;
}

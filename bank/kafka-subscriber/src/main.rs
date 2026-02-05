use domain::{account, transaction};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::subscriber;

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let ctx = subscriber::context().await;
    ctx.setup::<account::AccountCommand>().await;
    ctx.setup::<transaction::TransactionCommand>().await;

    ctx.all_done().await;
}

use domain::{account, transaction};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::projector;

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let ctx = projector::context().await;
    let mut guard = ctx.lock().unwrap();
    guard.subscribe::<account::Account>();
    guard.subscribe::<transaction::Transaction>();
    guard.launch();

    projector::App::all_done();
}

use std::sync::OnceLock;
use tracing::Level;
use tracing_subscriber::fmt;

static INIT_LOGGING: OnceLock<()> = OnceLock::new();

pub fn init() {
    INIT_LOGGING.get_or_init(|| fmt().with_test_writer().with_max_level(Level::TRACE).init());
}

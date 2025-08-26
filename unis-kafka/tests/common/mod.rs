use std::sync::OnceLock;
use tracing::Level;
use tracing_subscriber::fmt;

pub fn init() {
    static INIT_LOGGING: OnceLock<()> = OnceLock::new();
    INIT_LOGGING.get_or_init(|| fmt().with_test_writer().with_max_level(Level::TRACE).init());
}

use std::sync::Once;
use tracing_subscriber;

#[allow(dead_code)]
static INIT: Once = Once::new();

/// Initialize logging for tests. This function can be called multiple times safely
/// and will only initialize logging once globally.
///
/// Uses `RUST_LOG` environment variable to control log levels.
#[allow(dead_code)]
pub fn init_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .ok();
    });
}

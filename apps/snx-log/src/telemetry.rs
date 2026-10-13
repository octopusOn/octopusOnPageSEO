use tracing::Level;
use tracing_subscriber::EnvFilter;

/// Initializes structured logging to stderr.
///
/// `RUST_LOG` takes precedence over the CLI verbosity so users can target specific crates.
pub(crate) fn init(level: Level) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.as_str().to_ascii_lowercase()));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
}

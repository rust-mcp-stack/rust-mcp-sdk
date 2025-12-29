use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Set up the tracing subscriber for logging
pub fn initialize_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

//! `octopuson-platform-server`: the web version of the OctopusOn GEO-SEO Platform.

use std::sync::Arc;

use octopuson_core::http::HttpTransport;
use octopuson_platform_server::config::Config;
use octopuson_platform_server::{AppState, app, schedule};

/// Time limit of one gateway request during a sign-in.
const GATEWAY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    if let Err(message) = run().await {
        tracing::error!("{message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = Config::from_lookup(|name| std::env::var(name).ok())?;
    let gateway: Arc<dyn HttpTransport> = Arc::new(
        octopuson_net::ApiClient::new(&octopuson_platform_app::app_name(), GATEWAY_TIMEOUT, None)
            .map_err(|error| error.to_string())?,
    );
    let bind = config.bind;
    let state = Arc::new(AppState::new(config, gateway)?);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("{bind}: {error}"))?;
    tracing::info!(%bind, "GEO-SEO Platform listening");
    tokio::spawn(schedule::run(Arc::clone(&state)));
    let stopping = Arc::clone(&state);
    axum::serve(listener, app(Arc::clone(&state)))
        .with_graceful_shutdown(async move {
            stopped().await;
            stopping.accounts.stop_all();
        })
        .await
        .map_err(|error| error.to_string())
}

/// Waits for Ctrl-C, or for SIGTERM from systemd on Unix.
async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

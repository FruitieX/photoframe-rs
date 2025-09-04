mod config;
mod dither;
mod frame;
mod http;
mod pipeline;
mod scheduler;
mod sources;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config first so we can honor logging.filter directive.
    let shared = config::ConfigManager::load(None).await?;
    let cfg_snapshot = config::ConfigManager::to_struct(&shared).await?;
    let filter_directive = cfg_snapshot
        .logging
        .as_ref()
        .and_then(|l| l.filter.clone())
        .or_else(|| std::env::var("RUST_LOG").ok())
        .unwrap_or_else(|| "info,photoframe_server=debug".to_string());
    fmt()
        .with_env_filter(EnvFilter::new(filter_directive))
        .init();
    let scheduler = std::sync::Arc::new(scheduler::FrameScheduler::new(shared.clone()).await?);
    scheduler.populate().await?;
    scheduler.start().await?;
    let state = http::AppState {
        cfg: shared,
        scheduler: scheduler.clone(),
    };
    let app = http::router(state);
    http::serve(app).await?;
    Ok(())
}

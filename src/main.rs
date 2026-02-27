use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wvp_gb28181_server::{config::load_config, run};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,wvp_gb28181_server=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = load_config()?;
    run(cfg).await?;
    Ok(())
}

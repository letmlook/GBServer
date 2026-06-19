use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use gbserver::{config::load_config, run};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,gbserver=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = load_config()?;
    run(cfg).await?;
    Ok(())
}

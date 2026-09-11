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
        // 采集事件到内存队列，由 run() 中的后台任务落库到 gb_log，供 /api/log/list 查询
        .with(gbserver::logging::CaptureLayer)
        .init();

    let cfg = load_config()?;
    run(cfg).await?;
    Ok(())
}

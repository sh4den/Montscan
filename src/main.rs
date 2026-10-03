mod config;
mod env;

use tracing_subscriber::EnvFilter;

use config::Config;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cfg = Config::load();
    tracing::info!("Ollama: {} ({})", cfg.ollama.host, cfg.ollama.model);
}

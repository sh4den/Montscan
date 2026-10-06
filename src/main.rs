mod agent;
mod config;
mod env;

use tracing_subscriber::EnvFilter;

use config::Config;

pub fn http_client(accept_invalid_certs: bool) -> reqwest::Result<reqwest::Client> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(accept_invalid_certs)
        .build()
}

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

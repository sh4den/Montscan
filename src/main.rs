mod agent;
mod config;
mod env;
mod ingress;

use std::process::ExitCode;
use std::sync::Arc;

use ingress::Processor;
use tracing_subscriber::EnvFilter;

use agent::Agent;
use config::Config;

pub fn http_client(accept_invalid_certs: bool) -> reqwest::Result<reqwest::Client> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(accept_invalid_certs)
        .build()
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cfg = Config::load();
    let http = match http_client(false) {
        Ok(http) => http,
        Err(e) => {
            tracing::error!("Failed to build HTTP client: {e}");
            return ExitCode::FAILURE;
        }
    };
    let _processor: Processor = Arc::new(Agent::new(http, cfg.ollama));

    ExitCode::SUCCESS
}

mod agent;
mod banner;
mod config;
mod egress;
mod env;
mod ingress;

use std::process::ExitCode;
use std::sync::Arc;

use colored::Colorize;
use ingress::Processor;
use tokio::task::JoinSet;
use tracing_subscriber::EnvFilter;

use agent::Agent;
use banner::IngressEntry;
use config::Config;
use egress::Egress;

pub fn http_client(accept_invalid_certs: bool) -> reqwest::Result<reqwest::Client> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(accept_invalid_certs)
        .build()
}

fn ingresses(cfg: &Config) -> Vec<IngressEntry> {
    vec![
        IngressEntry {
            icon: "📡",
            enabled: cfg.ftp.enabled,
            flag: "FTP_ENABLED",
            ingress: Box::new(ingress::ftp::FtpIngress::new(cfg.ftp.clone())),
        },
        IngressEntry {
            icon: "🔐",
            enabled: cfg.sftp.enabled,
            flag: "SFTP_ENABLED",
            ingress: Box::new(ingress::sftp::SftpIngress::new(cfg.sftp.clone())),
        },
        IngressEntry {
            icon: "📤",
            enabled: cfg.scp.enabled,
            flag: "SCP_ENABLED",
            ingress: Box::new(ingress::scp::ScpIngress::new(cfg.scp.clone())),
        },
        IngressEntry {
            icon: "📂",
            enabled: cfg.folder.enabled,
            flag: "FOLDER_ENABLED",
            ingress: Box::new(ingress::folder::FolderIngress::new(cfg.folder.clone())),
        },
    ]
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,libunftp=warn,russh=warn")),
        )
        .with_target(false)
        .init();

    let cfg = Config::load();
    let entries = ingresses(&cfg);
    banner::print(&cfg, &entries);

    if !entries.iter().any(|e| e.enabled) {
        tracing::error!(
            "No ingress enabled. Enable FTP_ENABLED, SFTP_ENABLED, SCP_ENABLED, or FOLDER_ENABLED."
        );
        return ExitCode::FAILURE;
    }
    let http = match http_client(false) {
        Ok(http) => http,
        Err(e) => {
            tracing::error!("Failed to build HTTP client: {e}");
            return ExitCode::FAILURE;
        }
    };
    let egress = match Egress::from_config(&cfg, &http) {
        Ok(egress) => egress,
        Err(e) => {
            tracing::error!("Invalid egress configuration: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let processor: Processor = Arc::new(Agent::new(http, cfg.ollama, egress));

    let mut running = JoinSet::new();
    for entry in entries.into_iter().filter(|e| e.enabled) {
        let name = entry.ingress.name();
        let processor = processor.clone();
        running.spawn(async move { (name, entry.ingress.run(processor).await) });
    }

    println!(
        "{}",
        "🚀 Ingress services are now running! Press Ctrl+C to stop.".green()
    );
    println!();

    tokio::select! {
        _ = shutdown_signal() => {
            println!();
            println!("{}", "⏹️  Shutting down server...".yellow());
            tracing::info!("Server stopped by user");
            ExitCode::SUCCESS
        }
        Some(joined) = running.join_next() => {
            match joined {
                Ok((name, Ok(()))) => {
                    tracing::error!("{name} ingress stopped unexpectedly");
                }
                Ok((name, Err(e))) => {
                    println!("{}", format!("❌ Error starting server: {name}: {e:#}").red());
                    tracing::error!("{name} ingress failed: {e:#}");
                }
                Err(e) => tracing::error!("Ingress task panicked: {e}"),
            }
            ExitCode::FAILURE
        }
    }
}

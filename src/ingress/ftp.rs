use std::ops::RangeInclusive;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::env;
use crate::ingress::{Ingress, Processor, dispatch};
use anyhow::Context;
use async_trait::async_trait;
use libunftp::ServerBuilder;
use libunftp::notification::{DataEvent, DataListener, EventMeta};
use unftp_core::auth::{AuthenticationError, Authenticator, Credentials, Principal};
use unftp_sbe_fs::Filesystem;

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub upload_dir: PathBuf,
    pub passive_ports: RangeInclusive<u16>,
    pub passive_host: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            enabled: env::flag("FTP_ENABLED", true),
            host: env::string("FTP_HOST", "0.0.0.0"),
            port: env::parse("FTP_PORT", 21),
            username: env::string("FTP_USERNAME", "scanner"),
            password: env::string("FTP_PASSWORD", "scanner123"),
            upload_dir: env::string("FTP_UPLOAD_DIR", "./storage/inbox").into(),
            passive_ports: env::get("FTP_PASSIVE_PORTS")
                .and_then(|v| parse_port_range(&v))
                .unwrap_or(21000..=21010),
            passive_host: env::get("FTP_PASSIVE_HOST"),
        }
    }
}

fn parse_port_range(value: &str) -> Option<RangeInclusive<u16>> {
    let (start, end) = value.split_once('-')?;
    let (start, end) = (start.trim().parse().ok()?, end.trim().parse().ok()?);
    (start <= end).then_some(start..=end)
}

pub struct FtpIngress {
    config: Config,
}

impl FtpIngress {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Ingress for FtpIngress {
    fn name(&self) -> &'static str {
        "FTP"
    }

    fn summary(&self) -> Vec<(&'static str, String)> {
        let c = &self.config;
        vec![
            ("Host", c.host.clone()),
            ("Port", c.port.to_string()),
            ("Username", c.username.clone()),
            (
                "Passive ports",
                format!("{}-{}", c.passive_ports.start(), c.passive_ports.end()),
            ),
            (
                "Upload Directory",
                std::path::absolute(&c.upload_dir)
                    .unwrap_or(c.upload_dir.clone())
                    .display()
                    .to_string(),
            ),
        ]
    }

    async fn run(self: Box<Self>, processor: Processor) -> anyhow::Result<()> {
        let cfg = self.config;
        tokio::fs::create_dir_all(&cfg.upload_dir)
            .await
            .context("failed to create FTP upload directory")?;
        let root = std::path::absolute(&cfg.upload_dir)
            .context("failed to resolve FTP upload directory")?;

        let auth = Arc::new(SingleUser {
            username: cfg.username.clone(),
            password: cfg.password.clone(),
        });
        let fs_root = root.clone();
        let mut builder = ServerBuilder::with_authenticator(
            Box::new(move || Filesystem::new(fs_root.clone()).expect("FTP root is accessible")),
            auth,
        )
        .greeting("Welcome to Montscan FTP")
        .passive_ports(cfg.passive_ports.clone())
        .notify_data(UploadListener { root, processor });
        if let Some(host) = &cfg.passive_host {
            builder = builder.passive_host(host.as_str());
        }
        let server = builder
            .build()
            .context("invalid FTP server configuration")?;

        let addr = format!("{}:{}", cfg.host, cfg.port);
        tracing::info!("Starting FTP server on {addr}");
        server.listen(addr).await.context("FTP server stopped")?;
        Ok(())
    }
}

struct SingleUser {
    username: String,
    password: String,
}

impl std::fmt::Debug for SingleUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleUser")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[async_trait]
impl Authenticator for SingleUser {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<Principal, AuthenticationError> {
        if username != self.username {
            return Err(AuthenticationError::BadUser);
        }
        if creds.password.as_deref() != Some(self.password.as_str()) {
            return Err(AuthenticationError::BadPassword);
        }
        Ok(Principal {
            username: username.to_string(),
        })
    }
}

struct UploadListener {
    root: PathBuf,
    processor: Processor,
}

impl std::fmt::Debug for UploadListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadListener")
            .field("root", &self.root)
            .finish()
    }
}

#[async_trait]
impl DataListener for UploadListener {
    async fn receive_data_event(&self, event: DataEvent, _meta: EventMeta) {
        let ftp_path = match event {
            DataEvent::Put { path, .. } => path,
            DataEvent::Renamed { to, .. } => to,
            _ => return,
        };
        match resolve(&self.root, &ftp_path) {
            Some(local) => dispatch(&self.processor, "FTP", local),
            None => tracing::warn!("FTP: ignoring upload with unsafe path {ftp_path:?}"),
        }
    }
}

fn resolve(root: &Path, ftp_path: &str) -> Option<PathBuf> {
    let mut local = root.to_path_buf();
    for component in Path::new(ftp_path).components() {
        match component {
            Component::Normal(part) => local.push(part),
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    (local != root).then_some(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_port_ranges() {
        assert_eq!(parse_port_range("21000-21010"), Some(21000..=21010));
        assert_eq!(parse_port_range(" 1 - 2 "), Some(1..=2));
        assert_eq!(parse_port_range("10-1"), None);
        assert_eq!(parse_port_range("21000"), None);
    }

    #[test]
    fn resolves_inside_root_only() {
        let root = Path::new("/srv/scans");
        assert_eq!(resolve(root, "/scan.pdf"), Some(root.join("scan.pdf")));
        assert_eq!(
            resolve(root, "/in/./scan.pdf"),
            Some(root.join("in").join("scan.pdf"))
        );
        assert_eq!(resolve(root, "/../etc/passwd"), None);
        assert_eq!(resolve(root, "/"), None);
    }
}

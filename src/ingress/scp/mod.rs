mod protocol;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::env;
use crate::ingress::{Ingress, Processor, dispatch};
use anyhow::Context;
use async_trait::async_trait;
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub upload_dir: PathBuf,
    pub host_key: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            enabled: env::flag("SCP_ENABLED", false),
            host: env::string("SCP_HOST", "0.0.0.0"),
            port: env::parse("SCP_PORT", 2223),
            username: env::string("SCP_USERNAME", "scanner"),
            password: env::string("SCP_PASSWORD", "scanner123"),
            upload_dir: env::string("SCP_UPLOAD_DIR", "./storage/inbox").into(),
            host_key: env::string("SCP_HOST_KEY", "./storage/ssh/ssh_host_ed25519_key").into(),
        }
    }
}

pub struct ScpIngress {
    config: Config,
}

impl ScpIngress {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Ingress for ScpIngress {
    fn name(&self) -> &'static str {
        "SCP"
    }

    fn summary(&self) -> Vec<(&'static str, String)> {
        let c = &self.config;
        vec![
            ("Host", c.host.clone()),
            ("Port", c.port.to_string()),
            ("Username", c.username.clone()),
            (
                "Upload Directory",
                std::path::absolute(&c.upload_dir)
                    .unwrap_or(c.upload_dir.clone())
                    .display()
                    .to_string(),
            ),
            ("Host key", c.host_key.display().to_string()),
        ]
    }

    async fn run(self: Box<Self>, processor: Processor) -> anyhow::Result<()> {
        let cfg = self.config;
        tokio::fs::create_dir_all(&cfg.upload_dir)
            .await
            .context("failed to create SCP upload directory")?;
        let root = std::path::absolute(&cfg.upload_dir)
            .context("failed to resolve SCP upload directory")?;
        let key = crate::ingress::ssh::load_or_create_host_key(&cfg.host_key)?;

        let mut server = SshServer {
            shared: Arc::new(Shared {
                username: cfg.username,
                password: cfg.password,
                root,
                processor,
            }),
        };
        tracing::info!("Starting SCP server on {}:{}", cfg.host, cfg.port);
        server
            .run_on_address(
                crate::ingress::ssh::server_config(key),
                (cfg.host.as_str(), cfg.port),
            )
            .await
            .context("SCP server stopped")
    }
}

struct Shared {
    username: String,
    password: String,
    root: PathBuf,
    processor: Processor,
}

struct SshServer {
    shared: Arc<Shared>,
}

impl russh::server::Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, peer: Option<SocketAddr>) -> SshSession {
        tracing::debug!("SCP: connection from {peer:?}");
        SshSession {
            shared: self.shared.clone(),
            channels: HashMap::new(),
        }
    }

    fn handle_session_error(&mut self, error: anyhow::Error) {
        crate::ingress::ssh::log_session_error("SCP", &error);
    }
}

struct SshSession {
    shared: Arc<Shared>,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Handler for SshSession {
    type Error = anyhow::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if crate::ingress::ssh::credentials_match(
            &self.shared.username,
            &self.shared.password,
            user,
            password,
        ) {
            tracing::info!("SCP: user {user} logged in");
            Ok(Auth::Accept)
        } else {
            tracing::warn!("SCP: rejected login for {user}");
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command_line = String::from_utf8_lossy(data);
        let parsed = protocol::parse_command(&command_line);
        let (command, channel) = match (parsed, self.channels.remove(&channel_id)) {
            (Ok(command), Some(channel)) => (command, channel),
            (Err(reason), _) => {
                tracing::warn!("SCP: rejecting command {command_line:?}: {reason}");
                session.channel_failure(channel_id)?;
                return Ok(());
            }
            (Ok(_), None) => {
                session.channel_failure(channel_id)?;
                return Ok(());
            }
        };

        session.channel_success(channel_id)?;
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let (mut read_half, write_half) = channel.split();
            let mut writer = write_half.make_writer();
            let result = protocol::receive(
                &mut read_half.make_reader(),
                &mut writer,
                &shared.root,
                &command,
                |path| dispatch(&shared.processor, "SCP", path),
            )
            .await;
            let _ = writer.flush().await;
            drop(writer);
            let status = match result {
                Ok(()) => 0,
                Err(reason) => {
                    tracing::warn!("SCP: upload failed: {reason}");
                    1
                }
            };
            let _ = write_half.exit_status(status).await;
            let _ = write_half.eof().await;
            let _ = write_half.close().await;
        });
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            tracing::warn!(
                "SCP: client requested the SFTP subsystem; use `scp -O` for the legacy SCP protocol, or enable the SFTP ingress"
            );
        }
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(channel)?;
        Ok(())
    }
}

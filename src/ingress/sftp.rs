use std::collections::HashMap;
use std::io::SeekFrom;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::env;
use crate::ingress::{Ingress, Processor, dispatch, is_pdf};
use anyhow::Context;
use async_trait::async_trait;
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const MAX_READ_LEN: u32 = 256 * 1024;

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
            enabled: env::flag("SFTP_ENABLED", false),
            host: env::string("SFTP_HOST", "0.0.0.0"),
            port: env::parse("SFTP_PORT", 2222),
            username: env::string("SFTP_USERNAME", "scanner"),
            password: env::string("SFTP_PASSWORD", "scanner123"),
            upload_dir: env::string("SFTP_UPLOAD_DIR", "./storage/inbox").into(),
            host_key: env::string("SFTP_HOST_KEY", "./storage/ssh/ssh_host_ed25519_key").into(),
        }
    }
}

pub struct SftpIngress {
    config: Config,
}

impl SftpIngress {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Ingress for SftpIngress {
    fn name(&self) -> &'static str {
        "SFTP"
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
            .context("failed to create SFTP upload directory")?;
        let root = std::path::absolute(&cfg.upload_dir)
            .context("failed to resolve SFTP upload directory")?;
        let key = crate::ingress::ssh::load_or_create_host_key(&cfg.host_key)?;

        let mut server = SshServer {
            shared: Arc::new(Shared {
                username: cfg.username,
                password: cfg.password,
                root,
                processor,
            }),
        };
        tracing::info!("Starting SFTP server on {}:{}", cfg.host, cfg.port);
        server
            .run_on_address(
                crate::ingress::ssh::server_config(key),
                (cfg.host.as_str(), cfg.port),
            )
            .await
            .context("SFTP server stopped")
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
        tracing::debug!("SFTP: connection from {peer:?}");
        SshSession {
            shared: self.shared.clone(),
            channels: HashMap::new(),
        }
    }

    fn handle_session_error(&mut self, error: anyhow::Error) {
        crate::ingress::ssh::log_session_error("SFTP", &error);
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
            tracing::info!("SFTP: user {user} logged in");
            Ok(Auth::Accept)
        } else {
            tracing::warn!("SFTP: rejected login for {user}");
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

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match (name, self.channels.remove(&channel_id)) {
            ("sftp", Some(channel)) => {
                session.channel_success(channel_id)?;
                let handler = SftpSession::new(self.shared.clone());
                russh_sftp::server::run(channel.into_stream(), handler).await;
            }
            _ => session.channel_failure(channel_id)?,
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(channel)?;
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

enum OpenHandle {
    File {
        file: tokio::fs::File,
        path: PathBuf,
        written: bool,
    },
    Dir {
        path: PathBuf,
        listed: bool,
    },
}

struct SftpSession {
    shared: Arc<Shared>,
    handles: HashMap<String, OpenHandle>,
    next_handle: u64,
}

impl SftpSession {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            handles: HashMap::new(),
            next_handle: 0,
        }
    }

    fn local(&self, path: &str) -> Result<PathBuf, StatusCode> {
        crate::ingress::ssh::resolve(&self.shared.root, path).ok_or(StatusCode::PermissionDenied)
    }

    fn insert(&mut self, handle: OpenHandle) -> String {
        self.next_handle += 1;
        let id = self.next_handle.to_string();
        self.handles.insert(id.clone(), handle);
        id
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

fn io_status(error: std::io::Error) -> StatusCode {
    match error.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}

async fn attrs_of(path: &Path) -> Result<FileAttributes, StatusCode> {
    let meta = tokio::fs::metadata(path).await.map_err(io_status)?;
    Ok(FileAttributes::from(&meta))
}

impl russh_sftp::server::Handler for SftpSession {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let normalized = crate::ingress::ssh::normalize(&path).ok_or(StatusCode::NoSuchFile)?;
        Ok(Name {
            id,
            files: vec![File::dummy(normalized)],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let attrs = attrs_of(&self.local(&path)?).await?;
        Ok(Attrs { id, attrs })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.stat(id, path).await
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let path = match self.handles.get(&handle) {
            Some(OpenHandle::File { path, .. } | OpenHandle::Dir { path, .. }) => path.clone(),
            None => return Err(StatusCode::Failure),
        };
        Ok(Attrs {
            id,
            attrs: attrs_of(&path).await?,
        })
    }

    async fn setstat(
        &mut self,
        id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(ok(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        _handle: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(ok(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let local = self.local(&path)?;
        if !tokio::fs::metadata(&local)
            .await
            .map_err(io_status)?
            .is_dir()
        {
            return Err(StatusCode::NoSuchFile);
        }
        let handle = self.insert(OpenHandle::Dir {
            path: local,
            listed: false,
        });
        Ok(Handle { id, handle })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let Some(OpenHandle::Dir { path, listed }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        if *listed {
            return Err(StatusCode::Eof);
        }
        *listed = true;

        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(&*path).await.map_err(io_status)?;
        while let Some(entry) = entries.next_entry().await.map_err(io_status)? {
            if let Ok(meta) = entry.metadata().await {
                files.push(File::new(
                    entry.file_name().to_string_lossy(),
                    FileAttributes::from(&meta),
                ));
            }
        }
        Ok(Name { id, files })
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let local = self.local(&filename)?;
        if local == self.shared.root {
            return Err(StatusCode::Failure);
        }
        let options: std::fs::OpenOptions = pflags.into();
        let file = tokio::fs::OpenOptions::from(options)
            .open(&local)
            .await
            .map_err(io_status)?;
        let handle = self.insert(OpenHandle::File {
            file,
            path: local,
            written: false,
        });
        Ok(Handle { id, handle })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let Some(OpenHandle::File { file, .. }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(io_status)?;
        let mut data = vec![0; len.min(MAX_READ_LEN) as usize];
        let n = file.read(&mut data).await.map_err(io_status)?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(n);
        Ok(Data { id, data })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(OpenHandle::File { file, written, .. }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(io_status)?;
        file.write_all(&data).await.map_err(io_status)?;
        *written = true;
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        if let Some(OpenHandle::File {
            mut file,
            path,
            written: true,
        }) = self.handles.remove(&handle)
        {
            file.flush().await.map_err(io_status)?;
            drop(file);
            dispatch(&self.shared.processor, "SFTP", path);
        }
        Ok(ok(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        tokio::fs::remove_file(self.local(&filename)?)
            .await
            .map_err(io_status)?;
        Ok(ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        tokio::fs::create_dir(self.local(&path)?)
            .await
            .map_err(io_status)?;
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        let local = self.local(&path)?;
        if local == self.shared.root {
            return Err(StatusCode::PermissionDenied);
        }
        tokio::fs::remove_dir(local).await.map_err(io_status)?;
        Ok(ok(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        let (from, to) = (self.local(&oldpath)?, self.local(&newpath)?);
        tokio::fs::rename(&from, &to).await.map_err(io_status)?;
        if is_pdf(&to) && !is_pdf(&from) {
            dispatch(&self.shared.processor, "SFTP", to);
        }
        Ok(ok(id))
    }
}

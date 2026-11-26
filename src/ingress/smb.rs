use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::env;
use crate::ingress::{Ingress, Processor, dispatch};
use ::smb::{
    Client, ClientConfig, ConnectionConfig, DirAccessMask, Directory, File, FileAccessMask,
    FileCreateArgs, FileDirectoryInformation, FileDispositionInformation, GetLen, ReadAt, UncPath,
    WriteAt,
};
use anyhow::{Context, bail};
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

/// Read/write size every SMB2 server accepts.
const BASE_CHUNK: usize = 64 * 1024;
/// Upper bound for servers that support multi-credit (large MTU) requests.
const MAX_CHUNK: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub share: String,
    pub path: String,
    pub poll_interval: Duration,
    pub delete_after_read: bool,
    pub work_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |name: &str, default: &str| {
            env::get_either(
                &format!("SAMBA_SERVER_{name}"),
                &format!("SAMBA_INGRESS_{name}"),
            )
            .unwrap_or_else(|| default.to_string())
        };
        let ftp_upload_dir = env::string("FTP_UPLOAD_DIR", "./storage/inbox");
        Self {
            enabled: get("ENABLED", "false") == "true",
            host: get("HOST", "localhost"),
            port: get("PORT", "").parse().unwrap_or(445),
            username: get("USERNAME", ""),
            password: get("PASSWORD", ""),
            share: get("SHARE", "scans"),
            path: get("PATH", "/scans"),
            poll_interval: Duration::from_secs(
                get("POLL_INTERVAL_SEC", "")
                    .parse()
                    .ok()
                    .filter(|s| *s >= 1)
                    .unwrap_or(10),
            ),
            delete_after_read: get("DELETE_AFTER_READ", "true") == "true",
            work_dir: get("WORK_DIR", &ftp_upload_dir).into(),
        }
    }
}

/// An authenticated connection to a share, plus the largest read and write
/// sizes the server negotiated (so transfers take as few round trips as possible).
pub struct Share {
    pub client: Client,
    pub unc: UncPath,
    pub read_chunk: usize,
    pub write_chunk: usize,
}

impl Share {
    pub async fn close(self) {
        let _ = self.client.close().await;
    }
}

pub async fn connect(
    host: &str,
    port: u16,
    share: &str,
    username: &str,
    password: &str,
) -> ::smb::Result<Share> {
    let client = Client::new(ClientConfig {
        connection: ConnectionConfig {
            port: Some(port),
            ..Default::default()
        },
        ..Default::default()
    });
    let unc = UncPath::new(host)?.with_share(share)?;
    client
        .share_connect(&unc, username, password.to_string())
        .await?;
    let (read_chunk, write_chunk) = io_sizes(&client, host).await;
    Ok(Share {
        client,
        unc,
        read_chunk,
        write_chunk,
    })
}

async fn io_sizes(client: &Client, host: &str) -> (usize, usize) {
    let Ok(connection) = client.get_connection(host).await else {
        return (BASE_CHUNK, BASE_CHUNK);
    };
    let Some(info) = connection.conn_info() else {
        return (BASE_CHUNK, BASE_CHUNK);
    };
    let negotiated = &info.negotiation;
    if !negotiated.caps.large_mtu() {
        return (BASE_CHUNK, BASE_CHUNK);
    }
    (
        chunk_size(negotiated.max_read_size),
        chunk_size(negotiated.max_write_size),
    )
}

fn chunk_size(negotiated: u32) -> usize {
    match negotiated as usize {
        0 => BASE_CHUNK,
        n => n.min(MAX_CHUNK),
    }
}

pub fn share_path(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\\")
}

pub async fn write_all(file: &File, data: &[u8], chunk: usize) -> ::smb::Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + chunk).min(data.len());
        let written = file.write_at(&data[offset..end], offset as u64).await?;
        if written == 0 {
            return Err(::smb::Error::InvalidState(
                "server accepted zero bytes".into(),
            ));
        }
        offset += written;
    }
    Ok(())
}

pub struct SmbIngress {
    config: Config,
}

impl SmbIngress {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Ingress for SmbIngress {
    fn name(&self) -> &'static str {
        "Samba Server"
    }

    fn summary(&self) -> Vec<(&'static str, String)> {
        let c = &self.config;
        vec![
            ("Host", c.host.clone()),
            ("Port", c.port.to_string()),
            ("Share", c.share.clone()),
            ("Path", c.path.clone()),
            ("Poll interval", format!("{}s", c.poll_interval.as_secs())),
        ]
    }

    async fn run(self: Box<Self>, processor: Processor) -> anyhow::Result<()> {
        let cfg = self.config;
        if [
            &cfg.host,
            &cfg.username,
            &cfg.password,
            &cfg.share,
            &cfg.path,
        ]
        .iter()
        .any(|v| v.is_empty())
        {
            bail!("samba server configuration is incomplete");
        }

        let work_dir = cfg.work_dir.join("samba-server");
        tokio::fs::create_dir_all(&work_dir)
            .await
            .context("failed to create samba server work dir")?;

        tracing::info!(
            "Starting Samba server poller on {}:{} share={} path={} interval={}s",
            cfg.host,
            cfg.port,
            cfg.share,
            cfg.path,
            cfg.poll_interval.as_secs()
        );
        let mut ticker = tokio::time::interval(cfg.poll_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // One authenticated session is reused across polls; it is dropped and
        // re-established only after a poll fails.
        let mut connection: Option<Share> = None;
        loop {
            ticker.tick().await;
            let share = match connection {
                Some(ref share) => share,
                None => match connect(
                    &cfg.host,
                    cfg.port,
                    &cfg.share,
                    &cfg.username,
                    &cfg.password,
                )
                .await
                {
                    Ok(share) => connection.insert(share),
                    Err(e) => {
                        tracing::warn!(
                            "Samba server poll failed: failed to connect to SMB share: {e}"
                        );
                        continue;
                    }
                },
            };
            if let Err(e) = poll(&cfg, share, &work_dir, &processor).await {
                tracing::warn!("Samba server poll failed: {e:#}");
                if let Some(share) = connection.take() {
                    share.close().await;
                }
            }
        }
    }
}

async fn poll(
    cfg: &Config,
    share: &Share,
    work_dir: &std::path::Path,
    processor: &Processor,
) -> anyhow::Result<()> {
    let dir_path = share_path(&cfg.path);
    let names = list_pdfs(&share.client, &share.unc, &dir_path)
        .await
        .with_context(|| format!("failed to read smb path {:?}", cfg.path))?;

    for name in names {
        let remote = if dir_path.is_empty() {
            name.clone()
        } else {
            format!("{dir_path}\\{name}")
        };
        match download(share, &remote, &name, work_dir, cfg.delete_after_read).await {
            Ok(local) => dispatch(processor, "SMB", local),
            Err(e) => tracing::warn!("Failed to download SMB file {remote}: {e:#}"),
        }
    }
    Ok(())
}

async fn list_pdfs(
    client: &Client,
    share: &UncPath,
    dir_path: &str,
) -> anyhow::Result<Vec<String>> {
    let access = DirAccessMask::new()
        .with_list_directory(true)
        .with_synchronize(true);
    let dir = client
        .create_file(
            &share.clone().with_path(dir_path),
            &FileCreateArgs::make_open_existing(access.into()),
        )
        .await?
        .unwrap_dir();
    let dir = Arc::new(dir);

    let mut names = Vec::new();
    let mut entries = Directory::query::<FileDirectoryInformation>(&dir, "*").await?;
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let name = entry.file_name.to_string();
        if !entry.file_attributes.directory() && crate::ingress::is_pdf(std::path::Path::new(&name))
        {
            names.push(name);
        }
    }
    drop(entries);
    dir.close().await?;
    Ok(names)
}

async fn download(
    share: &Share,
    remote: &str,
    name: &str,
    work_dir: &std::path::Path,
    delete: bool,
) -> anyhow::Result<PathBuf> {
    let access = FileAccessMask::new()
        .with_generic_read(true)
        .with_file_read_data(true)
        .with_delete(delete);
    let file = share
        .client
        .create_file(
            &share.unc.clone().with_path(remote),
            &FileCreateArgs::make_open_existing(access),
        )
        .await?
        .unwrap_file();

    let result = async {
        let len = file.get_len().await?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let local = work_dir.join(format!("{nanos}-{name}"));
        let mut out = tokio::fs::File::create(&local).await?;

        let mut buf = vec![0u8; share.read_chunk];
        let mut offset = 0u64;
        while offset < len {
            let n = file.read_at(&mut buf, offset).await?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).await?;
            offset += n as u64;
        }
        out.flush().await?;
        if offset < len {
            drop(out);
            let _ = tokio::fs::remove_file(&local).await;
            bail!("short read: got {offset} of {len} bytes");
        }

        if delete && let Err(e) = file.set_info(FileDispositionInformation::default()).await {
            tracing::warn!("Failed to remove SMB source file {remote}: {e}");
        }
        Ok(local)
    }
    .await;
    let _ = file.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_negotiated_chunk_sizes() {
        assert_eq!(chunk_size(0), BASE_CHUNK);
        assert_eq!(chunk_size(8 * 1024 * 1024), MAX_CHUNK);
        assert_eq!(chunk_size(32 * 1024), 32 * 1024);
    }

    #[test]
    fn converts_share_paths() {
        assert_eq!(share_path("/scans"), "scans");
        assert_eq!(share_path("/a/b/"), "a\\b");
        assert_eq!(share_path("a\\b"), "a\\b");
        assert_eq!(share_path("/"), "");
    }
}

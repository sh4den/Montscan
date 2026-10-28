use std::path::Path;

use ::smb::{CreateOptions, FileAttributes, FileCreateArgs};
use anyhow::{Context, bail};

use crate::config::SambaConfig;

pub struct Samba {
    host: String,
    port: u16,
    share: String,
    username: String,
    password: String,
    path: String,
}

impl Samba {
    pub fn new(cfg: &SambaConfig) -> anyhow::Result<Self> {
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
            bail!("samba configuration is incomplete");
        }
        Ok(Self {
            host: cfg.host.clone(),
            port: cfg.port,
            share: cfg.share.clone(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            path: crate::ingress::smb::share_path(&cfg.path),
        })
    }

    pub async fn upload(&self, local: &Path, filename: &str) -> anyhow::Result<()> {
        let data = tokio::fs::read(local)
            .await
            .context("failed to open local file")?;
        let (client, share) = crate::ingress::smb::connect(
            &self.host,
            self.port,
            &self.share,
            &self.username,
            &self.password,
        )
        .await
        .context("failed to connect to SMB server")?;

        let remote = if self.path.is_empty() {
            filename.to_string()
        } else {
            format!("{}\\{filename}", self.path)
        };
        let result = async {
            let file = client
                .create_file(
                    &share.clone().with_path(&remote),
                    &FileCreateArgs::make_overwrite(FileAttributes::new(), CreateOptions::new()),
                )
                .await
                .context("failed to create remote file")?
                .unwrap_file();
            let written = crate::ingress::smb::write_all(&file, &data)
                .await
                .context("failed to write to remote file");
            let _ = file.close().await;
            written
        }
        .await;
        let _ = client.close().await;
        result?;

        tracing::info!("Successfully uploaded to Samba: {remote}");
        Ok(())
    }
}

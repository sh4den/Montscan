mod local;
mod webdav;

use std::path::{Path, PathBuf};

use crate::ingress::Outcome;

use crate::config::Config;

pub enum Egress {
    WebDav(webdav::WebDav),
    Local { output_dir: Option<PathBuf> },
}

impl Egress {
    pub fn from_config(cfg: &Config, http: &reqwest::Client) -> anyhow::Result<Option<Self>> {
        Ok(if cfg.webdav.enabled {
            Some(Self::WebDav(webdav::WebDav::new(
                &cfg.webdav,
                http.clone(),
            )?))
        } else if cfg.folder.enabled {
            Some(Self::Local {
                output_dir: cfg.folder.output_dir.clone(),
            })
        } else {
            None
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::WebDav(_) => "WebDAV",
            Self::Local { .. } => "local move",
        }
    }

    pub async fn deliver(&self, path: &Path, filename: &str) -> anyhow::Result<Outcome> {
        match self {
            Self::WebDav(webdav) => webdav.upload(path, filename).await?,
            Self::Local { output_dir } => {
                let dest = local::move_file(path, output_dir.as_deref(), filename).await?;
                return Ok(Outcome::MovedTo(dest));
            }
        }

        match tokio::fs::remove_file(path).await {
            Ok(()) => tracing::info!("Deleted processed file: {}", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("Error deleting file {}: {e}", path.display()),
        }
        Ok(Outcome::Uploaded)
    }
}

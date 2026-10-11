mod local;

use std::path::{Path, PathBuf};

use crate::ingress::Outcome;

use crate::config::Config;

pub enum Egress {
    Local { output_dir: Option<PathBuf> },
}

impl Egress {
    pub fn from_config(cfg: &Config) -> anyhow::Result<Option<Self>> {
        Ok(if cfg.folder.enabled {
            Some(Self::Local {
                output_dir: cfg.folder.output_dir.clone(),
            })
        } else {
            None
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Local { .. } => "local move",
        }
    }

    pub async fn deliver(&self, path: &Path, filename: &str) -> anyhow::Result<Outcome> {
        match self {
            Self::Local { output_dir } => {
                let dest = local::move_file(path, output_dir.as_deref(), filename).await?;
                Ok(Outcome::MovedTo(dest))
            }
        }
    }
}

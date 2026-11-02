mod ollama;
mod pdf;

use std::path::PathBuf;

use crate::ingress::{DocumentProcessor, Outcome};
use async_trait::async_trait;

use crate::config::OllamaConfig;
use crate::egress::Egress;

pub struct Agent {
    http: reqwest::Client,
    ollama: OllamaConfig,
    egress: Option<Egress>,
}

impl Agent {
    pub fn new(http: reqwest::Client, ollama: OllamaConfig, egress: Option<Egress>) -> Self {
        Self {
            http,
            ollama,
            egress,
        }
    }
}

#[async_trait]
impl DocumentProcessor for Agent {
    async fn process(&self, path: PathBuf) -> Outcome {
        tracing::info!("Processing document: {}", path.display());

        let image = match pdf::extract_first_page(&path).await {
            Ok(image) => image,
            Err(e) => {
                tracing::error!("Error extracting image from PDF: {e:#}");
                return Outcome::Failed;
            }
        };
        let filename = ollama::generate_filename(&self.http, &self.ollama, &image).await;

        let Some(egress) = &self.egress else {
            tracing::warn!(
                "No upload provider configured; leaving {} in place (suggested name: {filename})",
                path.display()
            );
            return Outcome::Kept;
        };

        match egress.deliver(&path, &filename).await {
            Ok(outcome) => {
                tracing::info!("Document processing completed successfully: {filename}");
                outcome
            }
            Err(e) => {
                tracing::error!("Failed to deliver via {}: {e:#}", egress.name());
                Outcome::Failed
            }
        }
    }
}

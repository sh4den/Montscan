mod ollama;
mod pdf;

use std::path::PathBuf;

use crate::ingress::{DocumentProcessor, Outcome};
use async_trait::async_trait;

use crate::config::OllamaConfig;

pub struct Agent {
    http: reqwest::Client,
    ollama: OllamaConfig,
}

impl Agent {
    pub fn new(http: reqwest::Client, ollama: OllamaConfig) -> Self {
        Self { http, ollama }
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

        tracing::info!(
            "Suggested name for {}: {filename} (no upload provider yet)",
            path.display()
        );
        Outcome::Kept
    }
}

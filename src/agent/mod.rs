mod ollama;
mod pdf;

use std::path::PathBuf;

use crate::ingress::{DocumentProcessor, Outcome};
use async_trait::async_trait;
use tokio::sync::Semaphore;

use crate::config::OllamaConfig;
use crate::egress::Egress;

pub struct Agent {
    http: reqwest::Client,
    ollama: OllamaConfig,
    egress: Option<Egress>,
    /// Bounds how many documents are rendered and sent to Ollama at once, so a
    /// batch of scans queues here instead of piling up on Ollama and timing out.
    slots: Semaphore,
}

impl Agent {
    pub fn new(
        http: reqwest::Client,
        ollama: OllamaConfig,
        egress: Option<Egress>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            http,
            ollama,
            egress,
            slots: Semaphore::new(max_concurrent.max(1)),
        }
    }
}

#[async_trait]
impl DocumentProcessor for Agent {
    async fn process(&self, path: PathBuf) -> Outcome {
        let _slot = match self.slots.try_acquire() {
            Ok(slot) => slot,
            Err(_) => {
                tracing::info!("Queued document: {}", path.display());
                self.slots
                    .acquire()
                    .await
                    .expect("semaphore is never closed")
            }
        };
        tracing::info!("Processing document: {}", path.display());

        let image = match pdf::extract_first_page(&path, self.ollama.image_max_size).await {
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

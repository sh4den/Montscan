pub mod folder;
pub mod ftp;
mod ssh;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Uploaded,
    MovedTo(PathBuf),
    Kept,
    Failed,
}

#[async_trait]
pub trait DocumentProcessor: Send + Sync {
    async fn process(&self, path: PathBuf) -> Outcome;
}

pub type Processor = Arc<dyn DocumentProcessor>;

#[async_trait]
pub trait Ingress: Send {
    fn name(&self) -> &'static str;

    fn summary(&self) -> Vec<(&'static str, String)>;

    async fn run(self: Box<Self>, processor: Processor) -> anyhow::Result<()>;
}

pub fn dispatch(processor: &Processor, source: &str, path: PathBuf) {
    if !is_pdf(&path) {
        tracing::info!("{source}: skipping non-PDF file {}", path.display());
        return;
    }
    tracing::info!("{source}: file received {}", path.display());
    let processor = processor.clone();
    tokio::spawn(async move {
        processor.process(path).await;
    });
}

pub fn is_pdf(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pdf_case_insensitively() {
        assert!(is_pdf(Path::new("scan.pdf")));
        assert!(is_pdf(Path::new("/a/b/SCAN.PDF")));
        assert!(!is_pdf(Path::new("scan.pdf.part")));
        assert!(!is_pdf(Path::new("pdf")));
    }
}

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::env;
use crate::ingress::{Ingress, Outcome, Processor, is_pdf};
use anyhow::Context;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub input_dir: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub poll_interval: Duration,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            enabled: env::flag("FOLDER_ENABLED", false),
            input_dir: env::string("FOLDER_INPUT_DIR", "./input").into(),
            output_dir: env::get("FOLDER_OUTPUT_DIR").map(PathBuf::from),
            poll_interval: Duration::from_secs(env::parse("FOLDER_POLL_INTERVAL_SEC", 5u64).max(1)),
        }
    }
}

pub struct FolderIngress {
    config: Config,
}

impl FolderIngress {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Ingress for FolderIngress {
    fn name(&self) -> &'static str {
        "Folder Watcher"
    }

    fn summary(&self) -> Vec<(&'static str, String)> {
        let output = match &self.config.output_dir {
            Some(dir) => std::path::absolute(dir)
                .unwrap_or(dir.clone())
                .display()
                .to_string(),
            None => "rename in place".to_string(),
        };
        vec![
            (
                "Input",
                std::path::absolute(&self.config.input_dir)
                    .unwrap_or(self.config.input_dir.clone())
                    .display()
                    .to_string(),
            ),
            ("Output", output),
            (
                "Poll interval",
                format!("{}s", self.config.poll_interval.as_secs()),
            ),
        ]
    }

    async fn run(self: Box<Self>, processor: Processor) -> anyhow::Result<()> {
        let cfg = self.config;
        tokio::fs::create_dir_all(&cfg.input_dir)
            .await
            .with_context(|| format!("failed to create {}", cfg.input_dir.display()))?;
        if let Some(out) = &cfg.output_dir {
            tokio::fs::create_dir_all(out)
                .await
                .with_context(|| format!("failed to create {}", out.display()))?;
        }

        tracing::info!(
            "Starting folder watcher: input={} interval={}s",
            cfg.input_dir.display(),
            cfg.poll_interval.as_secs()
        );

        let state: Arc<Mutex<State>> = Arc::default();
        let mut ticker = tokio::time::interval(cfg.poll_interval);
        loop {
            ticker.tick().await;
            poll(&cfg, &processor, &state).await;
        }
    }
}

#[derive(Default)]
struct State {
    in_progress: HashSet<PathBuf>,
    produced: HashMap<PathBuf, u64>,
    generation: u64,
}

async fn poll(cfg: &Config, processor: &Processor, state: &Arc<Mutex<State>>) {
    let generation = {
        let mut st = state.lock().unwrap();
        st.generation += 1;
        st.generation
    };
    let mut entries = match tokio::fs::read_dir(&cfg.input_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(
                "Folder watcher: error reading {}: {e}",
                cfg.input_dir.display()
            );
            return;
        }
    };

    let mut seen = HashSet::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let is_file = entry.file_type().await.is_ok_and(|t| t.is_file());
        if !is_file || !is_pdf(&path) {
            continue;
        }
        seen.insert(path.clone());
        {
            let mut st = state.lock().unwrap();
            if st.produced.contains_key(&path) || !st.in_progress.insert(path.clone()) {
                continue;
            }
        }

        let processor = processor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let outcome = processor.process(path.clone()).await;
            let mut st = state.lock().unwrap();
            st.in_progress.remove(&path);
            if let Outcome::MovedTo(dest) = outcome {
                let generation = st.generation;
                st.produced.insert(dest, generation);
            }
        });
    }

    state
        .lock()
        .unwrap()
        .produced
        .retain(|p, recorded| seen.contains(p) || *recorded >= generation);
}

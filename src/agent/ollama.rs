use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::config::{KeepAlive, OllamaConfig};

const PROMPT_TEMPLATE: &str = "Based on this scanned document image, generate a concise, descriptive filename (without extension).
The filename should:
- Be 3-6 words maximum
- In {language}
- Use underscores instead of spaces
- Be descriptive of the document's content
- Use uppercase letters
- Not include special characters except underscores and hyphens
- Include relevant name if mentioned in the document
- Include relevant date if mentioned in the document at the end of the filename (format: yyyy-mm-dd)

Example: INVOICE_AMAZON_JOHN_2023-11-15

Respond with ONLY the filename, nothing else.";

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [Message<'a>; 1],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
    keep_alive: &'a KeepAlive,
    options: Options,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: String,
    images: [&'a str; 1],
}

#[derive(Serialize)]
struct Options {
    num_ctx: u32,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
    #[serde(default)]
    done_reason: String,
    #[serde(default)]
    prompt_eval_count: u64,
    #[serde(default)]
    eval_count: u64,
    #[serde(default)]
    load_duration: u64,
    #[serde(default)]
    total_duration: u64,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
    #[serde(default)]
    thinking: String,
}

pub async fn generate_filename(
    http: &reqwest::Client,
    cfg: &OllamaConfig,
    image_b64: &str,
) -> String {
    tracing::info!("Generating filename with AI...");
    match request(http, cfg, image_b64).await {
        Ok(name) => {
            let filename = format!("{name}.pdf");
            tracing::info!("Agent suggested filename: {filename}");
            filename
        }
        Err(e) => {
            tracing::warn!("{e:#}");
            fallback_filename()
        }
    }
}

async fn request(
    http: &reqwest::Client,
    cfg: &OllamaConfig,
    image_b64: &str,
) -> anyhow::Result<String> {
    let body = ChatRequest {
        model: &cfg.model,
        messages: [Message {
            role: "user",
            content: PROMPT_TEMPLATE.replace("{language}", &cfg.language),
            images: [image_b64],
        }],
        stream: false,
        think: cfg.think,
        keep_alive: &cfg.keep_alive,
        options: Options {
            num_ctx: cfg.num_ctx,
        },
    };
    let url = format!("{}/api/chat", cfg.host.trim_end_matches('/'));
    let response = http
        .post(url)
        .timeout(cfg.timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                anyhow::anyhow!(
                    "Ollama did not answer within {}s; raise OLLAMA_TIMEOUT or use a faster model",
                    cfg.timeout.as_secs()
                )
            } else {
                anyhow::Error::new(e).context("Error calling Ollama")
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!("Ollama returned status {}: {text}", status.as_u16());
    }
    let parsed: ChatResponse = response
        .json()
        .await
        .context("Error decoding Ollama response")?;
    tracing::info!(
        "Ollama: {} prompt tokens, {} output tokens, {:.1}s total ({:.1}s loading model)",
        parsed.prompt_eval_count,
        parsed.eval_count,
        parsed.total_duration as f64 / 1e9,
        parsed.load_duration as f64 / 1e9,
    );
    let name = sanitize_filename(parsed.message.content.trim());
    if name.is_empty() {
        if parsed.done_reason == "length" {
            let thinking = parsed.message.thinking.chars().count();
            bail!(
                "Ollama ran out of context (OLLAMA_NUM_CTX={}) before answering \
                 ({thinking} characters spent thinking); raise OLLAMA_NUM_CTX \
                 or use a non-thinking model",
                cfg.num_ctx,
            );
        }
        bail!("Ollama returned an empty filename");
    }
    Ok(name)
}

pub fn fallback_filename() -> String {
    format!("scan_{}.pdf", chrono::Local::now().format("%Y%m%d_%H%M%S"))
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.strip_suffix(".pdf").unwrap_or(&cleaned);
    let cleaned = cleaned.strip_suffix(".PDF").unwrap_or(cleaned);
    cleaned.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_model_output() {
        assert_eq!(
            sanitize_filename("INVOICE_ACME_2024-01-02"),
            "INVOICE_ACME_2024-01-02"
        );
        assert_eq!(
            sanitize_filename("A/B\\C:D*E?F\"G<H>I|J"),
            "A_B_C_D_E_F_G_H_I_J"
        );
        assert_eq!(sanitize_filename("REPORT.pdf"), "REPORT");
        assert_eq!(sanitize_filename("REPORT.PDF"), "REPORT");
        assert_eq!(sanitize_filename("TWO\nLINES"), "TWO_LINES");
    }

    #[test]
    fn fallback_looks_like_a_scan_name() {
        let name = fallback_filename();
        assert!(name.starts_with("scan_") && name.ends_with(".pdf"));
        assert_eq!(name.len(), "scan_YYYYMMDD_HHMMSS.pdf".len());
    }

    #[test]
    fn serializes_chat_request() {
        let body = ChatRequest {
            model: "llava",
            messages: [Message {
                role: "user",
                content: "hi".into(),
                images: ["aGk="],
            }],
            stream: false,
            think: Some(false),
            keep_alive: &KeepAlive::Duration("30m".into()),
            options: Options { num_ctx: 8192 },
        };
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "model": "llava",
                "messages": [{"role": "user", "content": "hi", "images": ["aGk="]}],
                "stream": false,
                "think": false,
                "keep_alive": "30m",
                "options": {"num_ctx": 8192}
            })
        );
    }

    #[test]
    fn omits_think_when_left_to_the_model() {
        let body = ChatRequest {
            model: "llava",
            messages: [Message {
                role: "user",
                content: "hi".into(),
                images: ["aGk="],
            }],
            stream: false,
            think: None,
            keep_alive: &KeepAlive::Seconds(-1),
            options: Options { num_ctx: 8192 },
        };
        let value = serde_json::to_value(&body).unwrap();
        assert!(value.get("think").is_none());
        assert_eq!(value["keep_alive"], serde_json::json!(-1));
    }
}

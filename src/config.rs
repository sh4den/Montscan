use std::time::Duration;

use crate::env;

pub struct Config {
    pub max_concurrent_documents: usize,
    pub ollama: OllamaConfig,
    pub webdav: WebDavConfig,
    pub samba: SambaConfig,
    pub ftp: crate::ingress::ftp::Config,
    pub sftp: crate::ingress::sftp::Config,
    pub scp: crate::ingress::scp::Config,
    pub smb: crate::ingress::smb::Config,
    pub folder: crate::ingress::folder::Config,
}

pub struct OllamaConfig {
    pub host: String,
    pub model: String,
    pub num_ctx: u32,
    pub timeout: Duration,
    pub language: String,
    pub keep_alive: KeepAlive,
    pub think: Option<bool>,
    pub image_max_size: u32,
}

/// How long Ollama keeps the model loaded after a request. A bare number is
/// seconds (negative keeps it loaded forever); anything else is passed through
/// as a duration string such as `30m`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum KeepAlive {
    Seconds(i64),
    Duration(String),
}

impl KeepAlive {
    fn parse(value: &str) -> Self {
        value
            .parse()
            .map_or_else(|_| Self::Duration(value.to_string()), Self::Seconds)
    }
}

/// `true`/`false` force thinking on or off; anything else (e.g. `auto`) leaves
/// it to the model's default.
fn parse_think(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub struct WebDavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub path: String,
    pub insecure: bool,
}

pub struct SambaConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub share: String,
    pub username: String,
    pub password: String,
    pub path: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            max_concurrent_documents: env::parse("MAX_CONCURRENT_DOCUMENTS", 1usize).max(1),
            ollama: OllamaConfig {
                host: env::string("OLLAMA_HOST", "http://localhost:11434"),
                model: env::string("OLLAMA_MODEL", "qwen3-vl:8b"),
                num_ctx: env::parse("OLLAMA_NUM_CTX", 8192),
                timeout: Duration::from_secs(env::parse("OLLAMA_TIMEOUT", 120)),
                language: env::string("LANGUAGE", "english"),
                keep_alive: KeepAlive::parse(&env::string("OLLAMA_KEEP_ALIVE", "30m")),
                think: parse_think(&env::string("OLLAMA_THINK", "false")),
                image_max_size: env::parse("OLLAMA_IMAGE_MAX_SIZE", 1536u32).clamp(256, 8192),
            },
            webdav: WebDavConfig {
                enabled: env::flag("WEBDAV_ENABLED", false),
                url: env::string("WEBDAV_URL", ""),
                username: env::string("WEBDAV_USERNAME", ""),
                password: env::string("WEBDAV_PASSWORD", ""),
                path: env::string("WEBDAV_UPLOAD_PATH", "/Documents/Scanned"),
                insecure: env::flag("WEBDAV_INSECURE", false),
            },
            samba: SambaConfig {
                enabled: env::flag("SAMBA_ENABLED", false),
                host: env::string("SAMBA_HOST", "localhost"),
                port: env::parse("SAMBA_PORT", 445),
                share: env::string("SAMBA_SHARE", "scans"),
                username: env::string("SAMBA_USERNAME", ""),
                password: env::string("SAMBA_PASSWORD", ""),
                path: env::string("SAMBA_PATH", "scans"),
            },
            ftp: crate::ingress::ftp::Config::from_env(),
            sftp: crate::ingress::sftp::Config::from_env(),
            scp: crate::ingress::scp::Config::from_env(),
            smb: crate::ingress::smb::Config::from_env(),
            folder: crate::ingress::folder::Config::from_env(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keep_alive() {
        assert_eq!(KeepAlive::parse("30m"), KeepAlive::Duration("30m".into()));
        assert_eq!(KeepAlive::parse("-1"), KeepAlive::Seconds(-1));
        assert_eq!(
            serde_json::to_value(KeepAlive::Seconds(-1)).unwrap(),
            serde_json::json!(-1)
        );
        assert_eq!(
            serde_json::to_value(KeepAlive::Duration("30m".into())).unwrap(),
            serde_json::json!("30m")
        );
    }

    #[test]
    fn parses_think() {
        assert_eq!(parse_think("false"), Some(false));
        assert_eq!(parse_think("true"), Some(true));
        assert_eq!(parse_think("auto"), None);
    }
}

use std::time::Duration;

use crate::env;

pub struct Config {
    pub ollama: OllamaConfig,
    pub webdav: WebDavConfig,
    pub samba: SambaConfig,
    pub folder: crate::ingress::folder::Config,
}

pub struct OllamaConfig {
    pub host: String,
    pub model: String,
    pub num_ctx: u32,
    pub timeout: Duration,
    pub language: String,
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
            ollama: OllamaConfig {
                host: env::string("OLLAMA_HOST", "http://localhost:11434"),
                model: env::string("OLLAMA_MODEL", "llava"),
                num_ctx: env::parse("OLLAMA_NUM_CTX", 8192),
                timeout: Duration::from_secs(env::parse("OLLAMA_TIMEOUT", 120)),
                language: env::string("LANGUAGE", "english"),
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
            folder: crate::ingress::folder::Config::from_env(),
        }
    }
}

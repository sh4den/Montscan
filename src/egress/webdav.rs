use std::path::Path;

use anyhow::{Context, bail};
use reqwest::{Method, Response, StatusCode};
use url::Url;

use crate::config::WebDavConfig;

const DEBUG_BODY_LIMIT: usize = 4096;

pub struct WebDav {
    http: reqwest::Client,
    base: Url,
    username: String,
    password: String,
    path: Vec<String>,
}

impl WebDav {
    pub fn new(cfg: &WebDavConfig, http: reqwest::Client) -> anyhow::Result<Self> {
        if cfg.url.is_empty() || cfg.username.is_empty() || cfg.password.is_empty() {
            bail!("WebDAV configuration is incomplete");
        }
        let http = if cfg.insecure {
            tracing::warn!(
                "InsecureSkipVerify is enabled for WebDAV client. This is not recommended for production environments."
            );
            crate::http_client(true)?
        } else {
            http
        };
        Ok(Self {
            http,
            base: Url::parse(&cfg.url).context("invalid WEBDAV_URL")?,
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            path: cfg
                .path
                .split('/')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        })
    }

    fn url(&self, segments: &[String], trailing_slash: bool) -> anyhow::Result<Url> {
        let mut url = self.base.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("WEBDAV_URL cannot be a base URL"))?;
            path.pop_if_empty().extend(segments);
            if trailing_slash {
                path.push("");
            }
        }
        Ok(url)
    }

    pub async fn upload(&self, local: &Path, filename: &str) -> anyhow::Result<()> {
        if let Err(e) = self.mkdir_all().await {
            tracing::warn!("Could not create remote directory (may already exist): {e:#}");
        }

        let data = tokio::fs::read(local)
            .await
            .context("failed to read local file")?;
        let mut segments = self.path.clone();
        segments.push(filename.to_string());
        let url = self.url(&segments, false)?;
        tracing::info!("Uploading to WebDAV: {url}");

        let response = self
            .send(Method::PUT, url, Some(data))
            .await
            .context("failed to upload to WebDAV")?;
        if !response.status().is_success() {
            bail!("failed to upload to WebDAV: {}", response.status());
        }
        tracing::info!("Successfully uploaded to WebDAV: /{}", segments.join("/"));
        Ok(())
    }

    async fn mkdir_all(&self) -> anyhow::Result<()> {
        let mkcol = Method::from_bytes(b"MKCOL").expect("valid method");
        for depth in 1..=self.path.len() {
            let url = self.url(&self.path[..depth], true)?;
            let response = self.send(mkcol.clone(), url.clone(), None).await?;
            let status = response.status();
            if !status.is_success() && status != StatusCode::METHOD_NOT_ALLOWED {
                bail!("MKCOL {url} -> {status}");
            }
        }
        Ok(())
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
    ) -> anyhow::Result<Response> {
        let mut request = self
            .http
            .request(method.clone(), url.clone())
            .basic_auth(&self.username, Some(&self.password));
        if let Some(body) = body {
            request = request.body(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("WebDAV debug: {method} {url} failed: {e}");
                return Err(e.into());
            }
        };

        let status = response.status();
        let exists = method.as_str() == "MKCOL" && status == StatusCode::METHOD_NOT_ALLOWED;
        if status.as_u16() >= 400 && !exists {
            let body = response.bytes().await.unwrap_or_default();
            let shown = &body[..body.len().min(DEBUG_BODY_LIMIT)];
            tracing::warn!(
                "WebDAV debug: {method} {url} -> {}, response body: {}",
                status.as_u16(),
                String::from_utf8_lossy(shown)
            );
            bail!("{method} {url} -> {status}");
        }
        Ok(response)
    }
}

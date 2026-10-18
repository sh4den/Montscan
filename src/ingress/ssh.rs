use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, PrivateKey};
use russh::{MethodKind, MethodSet};

pub fn load_or_create_host_key(path: &Path) -> anyhow::Result<PrivateKey> {
    if path.exists() {
        return russh::keys::load_secret_key(path, None)
            .with_context(|| format!("failed to load SSH host key {}", path.display()));
    }

    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)
        .context("failed to generate SSH host key")?;
    let encoded = key
        .to_openssh(LineEnding::LF)
        .context("failed to encode SSH host key")?;

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(encoded.as_bytes())?;
            tracing::info!("Generated new SSH host key at {}", path.display());
            Ok(key)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            russh::keys::load_secret_key(path, None)
                .with_context(|| format!("failed to load SSH host key {}", path.display()))
        }
        Err(e) => {
            Err(e).with_context(|| format!("failed to write SSH host key {}", path.display()))
        }
    }
}

pub fn server_config(host_key: PrivateKey) -> Arc<russh::server::Config> {
    Arc::new(russh::server::Config {
        methods: MethodSet::from(&[MethodKind::Password][..]),
        keys: vec![host_key],
        auth_rejection_time: Duration::from_secs(1),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    })
}

pub fn log_session_error(ingress: &str, error: &anyhow::Error) {
    let hang_up = match error.downcast_ref::<russh::Error>() {
        Some(russh::Error::IO(e)) => e.kind() == std::io::ErrorKind::UnexpectedEof,
        Some(russh::Error::Disconnect | russh::Error::HUP) => true,
        _ => false,
    };
    if hang_up {
        tracing::debug!("{ingress}: client disconnected: {error:#}");
    } else {
        tracing::warn!("{ingress}: session error: {error:#}");
    }
}

pub fn credentials_match(expected_user: &str, expected_pass: &str, user: &str, pass: &str) -> bool {
    let user_ok = constant_time_eq(expected_user.as_bytes(), user.as_bytes());
    let pass_ok = constant_time_eq(expected_pass.as_bytes(), pass.as_bytes());
    user_ok & pass_ok
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for (i, x) in a.iter().enumerate() {
        diff |= usize::from(x ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => {
                let mut components = Path::new(name).components();
                match (components.next(), components.next()) {
                    (Some(Component::Normal(_)), None) => parts.push(name),
                    _ => return None,
                }
            }
        }
    }
    Some(format!("/{}", parts.join("/")))
}

pub fn to_local(root: &Path, virtual_path: &str) -> PathBuf {
    let mut local = root.to_path_buf();
    local.extend(virtual_path.split('/').filter(|s| !s.is_empty()));
    local
}

pub fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    normalize(path).map(|v| to_local(root, &v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_jails_paths() {
        assert_eq!(normalize("").as_deref(), Some("/"));
        assert_eq!(normalize(".").as_deref(), Some("/"));
        assert_eq!(normalize("scan.pdf").as_deref(), Some("/scan.pdf"));
        assert_eq!(normalize("/a/./b/../c").as_deref(), Some("/a/c"));
        assert_eq!(
            normalize("../../etc/passwd").as_deref(),
            Some("/etc/passwd")
        );
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_path_tricks() {
        assert_eq!(normalize("/C:/Windows"), None);
        assert_eq!(normalize("/a\\..\\..\\b"), None);
    }

    #[test]
    fn maps_into_root() {
        let root = Path::new("/srv/scans");
        assert_eq!(resolve(root, "/../x.pdf"), Some(root.join("x.pdf")));
        assert_eq!(resolve(root, "/"), Some(root.to_path_buf()));
    }

    #[test]
    fn compares_credentials() {
        assert!(credentials_match("u", "p", "u", "p"));
        assert!(!credentials_match("u", "p", "u", "px"));
        assert!(!credentials_match("u", "p", "x", "p"));
        assert!(!credentials_match("u", "pass", "u", ""));
    }

    #[test]
    fn host_key_is_stable_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys").join("host_key");
        let first = load_or_create_host_key(&path).unwrap();
        let second = load_or_create_host_key(&path).unwrap();
        assert_eq!(first.public_key(), second.public_key());
    }
}

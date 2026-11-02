use std::path::{Path, PathBuf};

use anyhow::Context;

pub async fn move_file(
    source: &Path,
    output_dir: Option<&Path>,
    filename: &str,
) -> anyhow::Result<PathBuf> {
    let dir = match output_dir {
        Some(dir) => dir.to_path_buf(),
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let dest = unique_destination(&dir.join(filename), source).await;
    if dest == source {
        return Ok(dest);
    }

    if tokio::fs::rename(source, &dest).await.is_err() {
        copy_and_delete(source, &dest).await?;
    }
    tracing::info!("Moved {} -> {}", source.display(), dest.display());
    Ok(dest)
}

async fn unique_destination(wanted: &Path, source: &Path) -> PathBuf {
    if wanted == source || !tokio::fs::try_exists(wanted).await.unwrap_or(false) {
        return wanted.to_path_buf();
    }
    let stem = wanted.file_stem().unwrap_or_default().to_string_lossy();
    let ext = wanted
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 1.. {
        let candidate = wanted.with_file_name(format!("{stem}_{n}{ext}"));
        if !tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return candidate;
        }
    }
    unreachable!()
}

async fn copy_and_delete(source: &Path, dest: &Path) -> anyhow::Result<()> {
    if let Err(e) = tokio::fs::copy(source, dest).await {
        let _ = tokio::fs::remove_file(dest).await;
        return Err(e).context("local move: copy");
    }
    tokio::fs::remove_file(source)
        .await
        .context("local move: remove source")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renames_in_place_without_clobbering() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("INVOICE.pdf");
        std::fs::write(&existing, "old").unwrap();
        let source = dir.path().join("scan001.pdf");
        std::fs::write(&source, "new").unwrap();

        let dest = move_file(&source, None, "INVOICE.pdf").await.unwrap();
        assert_eq!(dest, dir.path().join("INVOICE_1.pdf"));
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new");
        assert!(!source.exists());
    }

    #[tokio::test]
    async fn same_name_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("INVOICE.pdf");
        std::fs::write(&source, "data").unwrap();

        let dest = move_file(&source, None, "INVOICE.pdf").await.unwrap();
        assert_eq!(dest, source);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "data");
    }

    #[tokio::test]
    async fn moves_into_output_dir() {
        let input = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let source = input.path().join("scan.pdf");
        std::fs::write(&source, "data").unwrap();

        let dest = move_file(&source, Some(output.path()), "REPORT.pdf")
            .await
            .unwrap();
        assert_eq!(dest, output.path().join("REPORT.pdf"));
        assert!(!source.exists());
    }
}

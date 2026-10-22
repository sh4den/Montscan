use std::path::{Path, PathBuf};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_LINE: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub recursive: bool,
    pub target: String,
}

pub fn parse_command(command: &str) -> Result<Command, String> {
    let words = shell_words(command)?;
    let mut words = words.iter();
    match words.next() {
        Some(program) if program == "scp" || program.ends_with("/scp") => {}
        _ => return Err("only scp uploads are supported".into()),
    }

    let (mut to, mut recursive) = (false, false);
    let mut rest = Vec::new();
    for word in words.by_ref() {
        if word == "--" {
            break;
        }
        match word.strip_prefix('-') {
            Some(flags) if !flags.is_empty() && rest.is_empty() => {
                for flag in flags.chars() {
                    match flag {
                        't' => to = true,
                        'r' => recursive = true,
                        'f' => return Err("downloads are not supported".into()),
                        _ => {}
                    }
                }
            }
            _ => rest.push(word.clone()),
        }
    }
    rest.extend(words.cloned());

    if !to {
        return Err("only scp uploads (-t) are supported".into());
    }
    Ok(Command {
        recursive,
        target: if rest.is_empty() {
            ".".into()
        } else {
            rest.join(" ")
        },
    })
}

fn shell_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current: Option<String> = None;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if let Some(word) = current.take() {
                    words.push(word);
                }
            }
            '\'' => {
                let word = current.get_or_insert_default();
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => return Err("unterminated quote".into()),
                    }
                }
            }
            '"' => {
                let word = current.get_or_insert_default();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => word.extend(chars.next()),
                        Some(c) => word.push(c),
                        None => return Err("unterminated quote".into()),
                    }
                }
            }
            '\\' => current.get_or_insert_default().extend(chars.next()),
            c => current.get_or_insert_default().push(c),
        }
    }
    words.extend(current);
    Ok(words)
}

pub async fn receive<R, W>(
    reader: &mut R,
    writer: &mut W,
    root: &Path,
    command: &Command,
    mut on_file: impl FnMut(PathBuf),
) -> Result<(), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let result = run(reader, writer, root, command, &mut on_file).await;
    if let Err(message) = &result {
        let _ = writer
            .write_all(format!("\x02{message}\n").as_bytes())
            .await;
        let _ = writer.flush().await;
    }
    result
}

async fn run<R, W>(
    reader: &mut R,
    writer: &mut W,
    root: &Path,
    command: &Command,
    on_file: &mut impl FnMut(PathBuf),
) -> Result<(), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let target_virtual = command.target.trim_start_matches('~');
    let target = crate::ingress::ssh::resolve(root, target_virtual).ok_or("invalid target path")?;
    let target_is_dir = tokio::fs::metadata(&target).await.is_ok_and(|m| m.is_dir());
    let mut dirs: Vec<PathBuf> = Vec::new();

    ack(writer).await?;
    loop {
        let mut kind = [0u8; 1];
        match reader.read(&mut kind).await {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
        let line = read_line(reader).await?;
        match kind[0] {
            b'C' => {
                let (size, name) = parse_record(&line)?;
                let dest = match dirs.last() {
                    Some(dir) => dir.join(name),
                    None if target_is_dir => target.join(name),
                    None => target.clone(),
                };
                ack(writer).await?;
                receive_file(reader, &dest, size).await?;
                ack(writer).await?;
                on_file(dest);
            }
            b'D' => {
                if !command.recursive {
                    return Err("received directory without -r".into());
                }
                let (_, name) = parse_record(&line)?;
                let dir = match dirs.last() {
                    Some(dir) => dir.join(name),
                    None if target_is_dir => target.join(name),
                    None => target.clone(),
                };
                tokio::fs::create_dir_all(&dir)
                    .await
                    .map_err(|e| e.to_string())?;
                dirs.push(dir);
                ack(writer).await?;
            }
            b'E' => {
                dirs.pop();
                ack(writer).await?;
            }
            b'T' => ack(writer).await?,
            b'\x01' => tracing::warn!("SCP: client warning: {line}"),
            b'\x02' => return Err(format!("client error: {line}")),
            other => return Err(format!("unexpected protocol record {:?}", other as char)),
        }
    }
}

async fn ack<W: AsyncWrite + Unpin>(writer: &mut W) -> Result<(), String> {
    writer.write_all(b"\0").await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())
}

async fn read_line<R: AsyncRead + Unpin>(reader: &mut R) -> Result<String, String> {
    let mut line = Vec::new();
    loop {
        let byte = reader
            .read_u8()
            .await
            .map_err(|_| "connection closed mid-record")?;
        if byte == b'\n' {
            return String::from_utf8(line).map_err(|_| "record is not UTF-8".into());
        }
        if line.len() == MAX_LINE {
            return Err("protocol record too long".into());
        }
        line.push(byte);
    }
}

fn parse_record(line: &str) -> Result<(u64, &str), String> {
    let mut parts = line.splitn(3, ' ');
    let (Some(_mode), Some(size), Some(name)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(format!("malformed record {line:?}"));
    };
    let size = size
        .parse()
        .map_err(|_| format!("invalid size in {line:?}"))?;
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) if !name.contains('/') => Ok((size, name)),
        _ => Err(format!("invalid file name {name:?}")),
    }
}

async fn receive_file<R: AsyncRead + Unpin>(
    reader: &mut R,
    dest: &Path,
    size: u64,
) -> Result<(), String> {
    let result = async {
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| e.to_string())?;
        let copied = tokio::io::copy(&mut reader.take(size), &mut file)
            .await
            .map_err(|e| e.to_string())?;
        if copied != size {
            return Err("connection closed mid-file".to_string());
        }
        file.flush().await.map_err(|e| e.to_string())?;
        match reader.read_u8().await {
            Ok(0) => Ok(()),
            Ok(_) => Err("client reported an error after the file".to_string()),
            Err(_) => Err("connection closed after file".to_string()),
        }
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(dest).await;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upload_commands() {
        assert_eq!(
            parse_command("scp -t .").unwrap(),
            Command {
                recursive: false,
                target: ".".into()
            }
        );
        assert_eq!(
            parse_command("scp -v -r -d -t -- 'my scans'").unwrap(),
            Command {
                recursive: true,
                target: "my scans".into()
            }
        );
        assert_eq!(
            parse_command("/usr/bin/scp -rt /in").unwrap(),
            Command {
                recursive: true,
                target: "/in".into()
            }
        );
        assert!(parse_command("scp -f secret.pdf").is_err());
        assert!(parse_command("rm -rf /").is_err());
        assert!(parse_command("scp -t 'oops").is_err());
    }

    #[test]
    fn rejects_unsafe_record_names() {
        assert!(parse_record("0644 10 ../evil.pdf").is_err());
        assert!(parse_record("0644 10 a/b.pdf").is_err());
        assert!(parse_record("0644 10 ..").is_err());
        assert_eq!(
            parse_record("0644 10 my scan.pdf").unwrap(),
            (10, "my scan.pdf")
        );
    }

    async fn upload(
        command: &str,
        client_bytes: &[u8],
    ) -> (tempfile::TempDir, Result<(), String>, Vec<PathBuf>, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let command = parse_command(command).unwrap();
        let mut reader = client_bytes;
        let mut replies = Vec::new();
        let mut received = Vec::new();
        let result = receive(&mut reader, &mut replies, dir.path(), &command, |p| {
            received.push(p)
        })
        .await;
        (dir, result, received, replies)
    }

    #[tokio::test]
    async fn receives_a_file_into_the_target_directory() {
        let (dir, result, received, replies) =
            upload("scp -t .", b"T1 0 1 0\nC0644 5 scan.pdf\nhello\0").await;
        assert_eq!(result, Ok(()));
        assert_eq!(received, vec![dir.path().join("scan.pdf")]);
        assert_eq!(
            std::fs::read(dir.path().join("scan.pdf")).unwrap(),
            b"hello"
        );
        assert_eq!(replies, b"\0\0\0\0");
    }

    #[tokio::test]
    async fn renames_single_file_to_a_new_target_name() {
        let (dir, result, received, _) =
            upload("scp -t /renamed.pdf", b"C0644 2 a.pdf\nhi\0").await;
        assert_eq!(result, Ok(()));
        assert_eq!(received, vec![dir.path().join("renamed.pdf")]);
    }

    #[tokio::test]
    async fn receives_directories_recursively() {
        let (dir, result, received, _) =
            upload("scp -r -t .", b"D0755 0 batch\nC0644 1 a.pdf\nx\0E\n").await;
        assert_eq!(result, Ok(()));
        assert_eq!(received, vec![dir.path().join("batch").join("a.pdf")]);
    }

    #[tokio::test]
    async fn rejects_traversal_and_reports_error() {
        let (dir, result, received, replies) = upload("scp -t .", b"C0644 1 ../x.pdf\nx\0").await;
        assert!(result.is_err());
        assert!(received.is_empty());
        assert!(replies.ends_with(b"\n") && replies.contains(&2));
        assert!(!dir.path().parent().unwrap().join("x.pdf").exists());
    }

    #[tokio::test]
    async fn removes_truncated_files() {
        let (dir, result, received, _) = upload("scp -t .", b"C0644 10 scan.pdf\nshort").await;
        assert!(result.is_err());
        assert!(received.is_empty());
        assert!(!dir.path().join("scan.pdf").exists());
    }
}

<p align="center">
    <img src="https://github.com/user-attachments/assets/b82fb45f-f583-440e-a7f0-c6559125afcc" alt="Montscan" width="auto" />
</p>

<p align="center">
    <a href="https://github.com/sh4den/Montscan/blob/main/LICENSE" alt="License">
        <img src="https://img.shields.io/github/license/sh4den/Montscan" /></a>
    <a href="https://github.com/sh4den/Montscan/releases/latest" alt="Release">
        <img src="https://img.shields.io/github/v/release/sh4den/Montscan" /></a>
    <a href="https://github.com/sh4den/Montscan/actions/workflows/docker-publish.yml" alt="Docker build">
        <img src="https://img.shields.io/github/actions/workflow/status/sh4den/Montscan/docker-publish.yml?label=docker%20build" /></a>
    <a href="https://github.com/sh4den/Montscan/pkgs/container/montscan" alt="Docker image">
        <img src="https://img.shields.io/badge/image-ghcr.io%2Fsh4den%2Fmontscan-blue?logo=docker&logoColor=white" /></a>
    <a href="https://www.rust-lang.org/" alt="Rust">
        <img src="https://img.shields.io/badge/rust-1.92%2B-orange?logo=rust" /></a>
    <a href="https://ollama.com/" alt="Ollama">
        <img src="https://img.shields.io/badge/AI-Ollama-black?logo=ollama" /></a>
    <a href="https://www.conventionalcommits.org/" alt="Conventional Commits">
        <img src="https://img.shields.io/badge/commits-conventional-fe5196?logo=conventionalcommits&logoColor=white" /></a>
    <a href="https://github.com/sh4den/Montscan/commits/dev" alt="Last commit">
        <img src="https://img.shields.io/github/last-commit/sh4den/Montscan/dev" /></a>
    <a href="https://github.com/sh4den/Montscan/issues" alt="Issues">
        <img src="https://img.shields.io/github/issues/sh4den/Montscan" /></a>
    <a href="https://github.com/sh4den/Montscan/graphs/contributors" alt="Contributors">
        <img src="https://img.shields.io/github/contributors/sh4den/Montscan" /></a>
</p>

## Self-hosted naming and filing for scanned documents

**Montscan** receives scanned documents from your network scanner, reads them with a local Ollama vision model, gives each file a descriptive, dated name, and delivers it wherever you keep your documents. It speaks FTP, SFTP, SCP and SMB, so most scanners work with it without extra software.

Naming runs locally through Ollama, so your documents are never sent to a third-party AI service and no API key is needed.

---

**[Install](#installation) &nbsp;|&nbsp; [Docker image](https://github.com/sh4den/Montscan/pkgs/container/montscan) &nbsp;|&nbsp; [Configuration](#configuration) &nbsp;|&nbsp; [Issues](https://github.com/sh4den/Montscan/issues) &nbsp;|&nbsp; [Contributing](CONTRIBUTING.md)**

---

> [!NOTE]
> Montscan is still in active development and not fully production-ready, but it is usable day to day.
> On a test set of 1000 documents it named 97.5% of them correctly.

If it's useful to you, a star on GitHub is appreciated.

## Features

- Receives scans over FTP, SFTP or legacy SCP, by polling a remote SMB share, or by watching a local folder
- Reads each document with an Ollama vision model and generates a dated, descriptive filename
- Delivers the result to WebDAV (Nextcloud, ownCloud, etc.), an SMB/CIFS share, or a local output folder (or renames it in place)
- Ships as a single binary and a Docker Compose setup

## Contents

- [Rust rewrite](#rust-rewrite)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [Docker](#docker)
- [Troubleshooting](#troubleshooting)
- [License](#license)

## Rust rewrite

Montscan was originally written in Go. Version 2.0.0 is a full rewrite in Rust. I wanted a single self-contained binary, clearer separation between components, and an easier way to add new protocols for receiving scans.

### What changed

- PDF pages are now rendered in-process with [`hayro`](https://crates.io/crates/hayro). The Go version shelled out to Poppler (`pdftoppm`) or ImageMagick; now the `montscan` binary is all you need at runtime.
- Documents can now arrive over SFTP and legacy SCP, in addition to FTP, Samba polling and the folder watcher.
- Every ingress implements a common `Ingress` trait and only passes finished PDFs to the document processor. Delivery (WebDAV, Samba, local move) lives in a separate `egress` module, so adding a protocol doesn't touch the AI or upload code. See [Architecture](#architecture).
- The FTP passive port range and advertised address are configurable (`FTP_PASSIVE_PORTS`, `FTP_PASSIVE_HOST`), which fixes most problems behind NAT or Docker.
- `OLLAMA_TIMEOUT` controls how long Montscan waits for a filename before falling back to `scan_<timestamp>.pdf`. Raise it for slower "thinking" models.
- The Docker image no longer installs `poppler-utils` and is smaller as a result.

### Upgrading from 1.x

> [!WARNING]
> 2.0.0 is a breaking release and is not backward compatible with 1.x.
> The Go version is kept on the [`legacy-v1.0.0`](https://github.com/sh4den/Montscan/tree/legacy-v1.0.0) branch and no longer receives updates.

- Environment variable names are unchanged. The old `SAMBA_INGRESS_*` names still work as fallbacks for `SAMBA_SERVER_*`.
- Uploads to WebDAV and Samba, and files received over FTP, now overwrite an existing file with the same name. 1.x appended `_1`, `_2`, and so on. Local moves by the folder watcher still never overwrite.
- Runtime files now live under `storage/`. Received scans default to `./storage/inbox` (was `./scans`) and SSH host keys go in `./storage/ssh`. If you relied on the old location, set `FTP_UPLOAD_DIR=./scans` or move the folder.
- Building from source needs Rust 1.92 or newer (`cargo build --release`).
- For Docker, pull the new image (`docker-compose pull`) or rebuild it. If you enable SFTP or SCP, publish ports `2222`/`2223` and mount `./storage:/app/storage` so scans and the generated host key survive container recreation. The included `docker-compose.yml` already does this.
- Logging is controlled by `RUST_LOG` (default `info`).

### On AI assistance

I used AI tools during the rewrite, mostly to split the work into commits and to look for bugs and edge cases. I reviewed every change before merging it.

## Architecture

Montscan is a single Rust crate with all code in `src/`. It doesn't need any external programs or C libraries at runtime. Each ingress is a module under `src/ingress/`, built on an existing protocol library, and implements the `Ingress` trait. An ingress only hands finished PDFs to a `DocumentProcessor` and knows nothing about naming or delivery.

| Module | Role | Built on |
|---|---|---|
| `ingress` | `Ingress` / `DocumentProcessor` traits | |
| `ingress::ftp` | FTP ingress | [`libunftp`](https://crates.io/crates/libunftp) + [`unftp-sbe-fs`](https://crates.io/crates/unftp-sbe-fs) |
| `ingress::sftp` | SFTP ingress | [`russh`](https://crates.io/crates/russh) + [`russh-sftp`](https://crates.io/crates/russh-sftp) |
| `ingress::scp` | SCP ingress | [`russh`](https://crates.io/crates/russh); there is no SCP server crate, so the sink side of the protocol is implemented in the module |
| `ingress::smb` | SMB share poller | [`smb`](https://crates.io/crates/smb), a pure Rust SMB2/3 client |
| `ingress::folder` | Local folder watcher | `tokio` |
| `ingress::ssh` | Host key, auth and path jailing shared by SFTP and SCP | `russh` |
| `agent` | PDF rendering and Ollama filename generation | [`hayro`](https://crates.io/crates/hayro), `reqwest`, `image` |
| `egress` | WebDAV / Samba / local delivery | `reqwest`, `smb` |
| `config`, `env`, `banner` | Settings and startup banner | |

## Prerequisites

- Rust 1.92 or newer, if you build from source
- [Ollama](https://ollama.ai/) with a vision model such as `llava` or `llama3.2-vision`
- Optionally, a WebDAV server or SMB share to deliver files to

## Installation

```bash
git clone https://github.com/sh4den/Montscan.git
cd Montscan
cargo build --release   # produces target/release/montscan
```

Install Ollama and pull a vision-capable model:

```bash
ollama pull llava
```

## Configuration

Montscan is configured through environment variables. You need at least one ingress enabled. You should also configure an egress, unless you use the folder watcher, which can rename or move files on its own. Egress is picked in this order: WebDAV, then Samba, then a local move if the folder watcher is enabled. With no egress, received files stay where they landed.

### Ingress: FTP server

| Variable | Description | Default |
|---|---|---|
| `FTP_ENABLED` | Enable the FTP server | `true` |
| `FTP_HOST` | Bind address | `0.0.0.0` |
| `FTP_PORT` | Port | `21` |
| `FTP_USERNAME` | Username | `scanner` |
| `FTP_PASSWORD` | Password | `scanner123` |
| `FTP_UPLOAD_DIR` | Local directory for received files | `./storage/inbox` |
| `FTP_PASSIVE_PORTS` | Passive data port range | `21000-21010` |
| `FTP_PASSIVE_HOST` | Address advertised for passive connections (set this to the host's LAN IP behind Docker/NAT) | *(control connection address)* |

A file is processed once its `STOR` completes, or when an upload is renamed to `*.pdf`.

### Ingress: SFTP server

| Variable | Description | Default |
|---|---|---|
| `SFTP_ENABLED` | Enable the SFTP server | `false` |
| `SFTP_HOST` | Bind address | `0.0.0.0` |
| `SFTP_PORT` | Port | `2222` |
| `SFTP_USERNAME` | Username (password auth only, no shell) | `scanner` |
| `SFTP_PASSWORD` | Password | `scanner123` |
| `SFTP_UPLOAD_DIR` | Directory clients see as `/` | `./storage/inbox` |
| `SFTP_HOST_KEY` | Ed25519 host key, generated on first start | `./storage/ssh/ssh_host_ed25519_key` |

A file is processed when the client closes it, or when an upload is renamed to `*.pdf`. Modern `scp` (OpenSSH 9+) actually uses SFTP, so it works against this port.

### Ingress: SCP server

| Variable | Description | Default |
|---|---|---|
| `SCP_ENABLED` | Enable the SCP server | `false` |
| `SCP_HOST` | Bind address | `0.0.0.0` |
| `SCP_PORT` | Port | `2223` |
| `SCP_USERNAME` | Username (password auth only, no shell) | `scanner` |
| `SCP_PASSWORD` | Password | `scanner123` |
| `SCP_UPLOAD_DIR` | Directory clients see as `/` | `./storage/inbox` |
| `SCP_HOST_KEY` | Ed25519 host key, generated on first start (can be shared with SFTP) | `./storage/ssh/ssh_host_ed25519_key` |

This is the legacy SCP protocol (`scp -t`), which is what most scanners and `scp -O` use. Only uploads are supported; recursive (`-r`) uploads work.

### Ingress: Samba server (polls a remote SMB share)

| Variable | Description | Default |
|---|---|---|
| `SAMBA_SERVER_ENABLED` | Enable the Samba poller | `false` |
| `SAMBA_SERVER_HOST` | SMB server hostname or IP | `localhost` |
| `SAMBA_SERVER_PORT` | SMB port | `445` |
| `SAMBA_SERVER_USERNAME` | SMB username | |
| `SAMBA_SERVER_PASSWORD` | SMB password | |
| `SAMBA_SERVER_SHARE` | Share name | `scans` |
| `SAMBA_SERVER_PATH` | Path within the share to watch | `/scans` |
| `SAMBA_SERVER_POLL_INTERVAL_SEC` | Polling interval in seconds | `10` |
| `SAMBA_SERVER_DELETE_AFTER_READ` | Delete the file from the share after downloading it | `true` |
| `SAMBA_SERVER_WORK_DIR` | Local staging directory | *(`FTP_UPLOAD_DIR`)* |

### Ingress: folder watcher

| Variable | Description | Default |
|---|---|---|
| `FOLDER_ENABLED` | Enable the folder watcher | `false` |
| `FOLDER_INPUT_DIR` | Directory to watch for PDFs | `./input` |
| `FOLDER_OUTPUT_DIR` | Where renamed files go; leave unset to rename in place | |
| `FOLDER_POLL_INTERVAL_SEC` | Polling interval in seconds | `5` |

### Egress: WebDAV

| Variable | Description | Default |
|---|---|---|
| `WEBDAV_ENABLED` | Enable WebDAV upload | `false` |
| `WEBDAV_URL` | Server base URL | |
| `WEBDAV_USERNAME` | Username | |
| `WEBDAV_PASSWORD` | Password | |
| `WEBDAV_UPLOAD_PATH` | Remote folder for uploads | `/Documents/Scanned` |
| `WEBDAV_INSECURE` | Skip TLS certificate verification | `false` |

### Egress: Samba (uploads to an SMB share)

| Variable | Description | Default |
|---|---|---|
| `SAMBA_ENABLED` | Enable Samba upload | `false` |
| `SAMBA_HOST` | SMB server hostname or IP | `localhost` |
| `SAMBA_PORT` | SMB port | `445` |
| `SAMBA_USERNAME` | SMB username | |
| `SAMBA_PASSWORD` | SMB password | |
| `SAMBA_SHARE` | Destination share | `scans` |
| `SAMBA_PATH` | Path within the share | `scans` |

### Naming (Ollama)

| Variable | Description | Default |
|---|---|---|
| `OLLAMA_HOST` | Ollama URL | `http://localhost:11434` |
| `OLLAMA_MODEL` | Vision model | `qwen3-vl:8b` |
| `OLLAMA_NUM_CTX` | Context window (tokens) requested per call | `8192` |
| `OLLAMA_TIMEOUT` | Seconds to wait before falling back to a timestamped name | `120` |
| `LANGUAGE` | Language of the generated filenames | `english` |

## Usage

### Example setups

Scanner to FTP, delivered to WebDAV:

```bash
FTP_ENABLED=true
FTP_USERNAME=scanner
FTP_PASSWORD=secret
WEBDAV_ENABLED=true
WEBDAV_URL=https://cloud.example.com/remote.php/webdav
WEBDAV_USERNAME=user
WEBDAV_PASSWORD=secret
```

Scanner writes to a Samba share that is mounted locally. Montscan watches the mount, names each PDF and moves it to an output folder:

```bash
FOLDER_ENABLED=true
FOLDER_INPUT_DIR=/mnt/scanner       # Samba mount point
FOLDER_OUTPUT_DIR=/mnt/documents    # where named files end up
FOLDER_POLL_INTERVAL_SEC=5
FTP_ENABLED=false
```

Same thing, but renaming files where they are:

```bash
FOLDER_ENABLED=true
FOLDER_INPUT_DIR=/mnt/scanner
# no FOLDER_OUTPUT_DIR, so files are renamed in place
FTP_ENABLED=false
```

### Running locally

```bash
cp .env.example .env
# edit .env
export $(grep -v '^#' .env | xargs)
./target/release/montscan
```

Logging defaults to `info`. Set `RUST_LOG=debug` for more detail.

## Docker

### Docker Compose

Set your environment variables in `docker-compose.yml`, then start it:

```bash
docker-compose up -d
```

This uses the prebuilt image from GitHub Container Registry (`ghcr.io/sh4den/montscan`). To build from source instead, uncomment the `build: .` line in `docker-compose.yml` and run `docker-compose up -d --build`.

Ollama runs on CPU by default. To use an NVIDIA GPU, add this to `.env`:

```bash
COMPOSE_PROFILES=gpu
OLLAMA_GPU_COUNT=1
```

Leave `COMPOSE_PROFILES=cpu` (or unset) for CPU only.

Follow the logs with `docker-compose logs -f` and stop everything with `docker-compose down`.

### Plain Docker

```bash
docker build -t montscan .

docker run -d \
  -p 21:21 \
  -p 21000-21010:21000-21010 \
  -v ./input:/app/input \
  -v ./output:/app/output \
  -e FOLDER_ENABLED=true \
  -e FOLDER_INPUT_DIR=/app/input \
  -e FOLDER_OUTPUT_DIR=/app/output \
  -e FTP_ENABLED=false \
  -e OLLAMA_HOST=http://host.docker.internal:11434 \
  --name montscan \
  montscan
```

## Troubleshooting

**FTP connection refused.** Make sure port 21 isn't blocked by a firewall. On Windows, allow Montscan through Windows Defender Firewall.

**FTP uploads hang after login (Docker).** Passive data connections have to reach the container. Publish the `FTP_PASSIVE_PORTS` range and set `FTP_PASSIVE_HOST` to the Docker host's LAN IP.

**SCP fails with "subsystem request failed".** OpenSSH 9+ `scp` uses SFTP by default. Use `scp -O` against the SCP port, or point the client at the SFTP port.

**SSH host key changed warning.** The host key is generated on first start at `SFTP_HOST_KEY` / `SCP_HOST_KEY`. In Docker, keep it on the `./storage` volume so it isn't regenerated when the container is recreated.

**Folder watcher doesn't pick up files.** Check that `FOLDER_INPUT_DIR` is the path inside the container (with a volume mounted to it) and that the process can read it. If your scanner writes slowly, increase `FOLDER_POLL_INTERVAL_SEC`.

**Files get a `scan_YYYYMMDD_HHMMSS.pdf` name.** That's the fallback when naming fails. Check that Ollama is running (`ollama list`) and that the configured model supports vision. For slow models, raise `OLLAMA_TIMEOUT`.

**WebDAV upload fails.** Double-check the URL and credentials, and make sure the upload path exists on the server. For Nextcloud, the URL should end with `/remote.php/webdav`. With a self-signed certificate, set `WEBDAV_INSECURE=true`.

**"failed to parse PDF".** Encrypted (password-protected) PDFs aren't supported. Turn off PDF encryption on the scanner.

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgments

Montscan is built on these projects:

- [libunftp](https://github.com/bolcom/libunftp), async FTP server
- [russh](https://github.com/Eugeny/russh) and [russh-sftp](https://github.com/AspectUnk/russh-sftp), SSH and SFTP servers
- [smb-rs](https://github.com/afiffon/smb-rs), pure Rust SMB2/3 client
- [Ollama](https://ollama.ai/), local model runner
- [reqwest](https://github.com/seanmonstar/reqwest), HTTP client for Ollama and WebDAV

Questions and bug reports are welcome as GitHub issues.

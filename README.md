> [!NOTE] 
> Montscan is not fully production-ready and is currently in active development but is fairly usable.</br> 
> It achieve **97.5%** success rate on a small test set of 1000 documents.

<div align="center">

  ![510231426-ced68675-3338-4e7e-b9a0-5a9fc887aeac-2](https://github.com/user-attachments/assets/b82fb45f-f583-440e-a7f0-c6559125afcc)
  
</div>

<div align="center">
  <p><b>Montscan</b>: Automated scanner document processor with Vision AI, AI naming, and flexible upload!</p>
</div>

<p align="center">
  ⭐ If you find <b>Montscan</b> useful, please consider giving it a star it really helps the project grow!
</p>

---

## ✨ Features

- 📡 **FTP Server** - Receives documents from network scanners via FTP
- 🔐 **SFTP Server** - Receives documents over SSH file transfer
- 📤 **SCP Server** - Receives documents via the legacy SCP protocol
- 📥 **Samba Server** - Polls a remote SMB share for incoming PDFs
- 📂 **Folder Watcher** - Monitors a local directory; renames in place or moves to an output folder
- 👁️ **Vision AI Processing** - Analyzes scanned documents using Ollama vision models
- 🤖 **AI-Powered Naming** - Generates descriptive, date-aware filenames via Ollama
- ☁️ **WebDAV Integration** - Uploads processed documents to Nextcloud, ownCloud, and any WebDAV server
- 🗂️ **Samba Provider** - Delivers processed documents to an SMB/CIFS network share
- 🐳 **Docker Support** - Easy deployment with Docker Compose

---

## 🦀 Rust Rewrite

Montscan was originally written in Go. Starting with **v0.2.0** it has been rewritten from the ground up in Rust. The goal was a single self-contained binary, cleaner boundaries between components, and room to add new ways for scanners to send documents.

### What changed

- **No more external PDF tools** - The Go version shelled out to Poppler (`pdftoppm`) or ImageMagick to turn PDFs into images. Pages are now rendered in-process with [`hayro`](https://crates.io/crates/hayro), so nothing beyond the `montscan` binary is needed at runtime.
- **New ingresses** - Montscan can now receive documents over **SFTP** and the legacy **SCP** protocol, alongside FTP, Samba polling and the folder watcher.
- **Pluggable architecture** - Every ingress implements a common `Ingress` trait and only hands finished PDFs to the document processor. Delivery (WebDAV, Samba, local move) lives in a separate `egress` module. Adding a new protocol no longer touches the AI or upload code. See [Architecture](#️-architecture).
- **Better FTP behind NAT/Docker** - The passive port range and advertised address can now be configured (`FTP_PASSIVE_PORTS`, `FTP_PASSIVE_HOST`).
- **Ollama timeout** - `OLLAMA_TIMEOUT` sets how long Montscan waits for a filename before falling back to `scan_<timestamp>.pdf`. This is useful for slower "thinking" vision models.
- **Smaller Docker image** - The runtime image no longer installs `poppler-utils`.

### Upgrading from the Go version

- **Configuration is compatible** - All existing environment variables keep their names and defaults. The legacy `SAMBA_INGRESS_*` names are still accepted as fallbacks for `SAMBA_SERVER_*`.
- **Building from source** now requires Rust 1.92+ instead of Go (`cargo build --release`).
- **Docker** - Rebuild the image. If you enable SFTP or SCP, publish ports `2222`/`2223` and mount `./data:/app/data` so the generated SSH host key survives container recreation. The provided `docker-compose.yml` already does both.
- **Logging** now uses `RUST_LOG` (default `info`).

### A note on AI assistance

AI tools helped during the rewrite, mainly to organize the work into commits and to audit the code for bugs and edge cases. Every change was reviewed before it was merged.

---

## 📋 Table of Contents

- [Rust Rewrite](#-rust-rewrite)
- [Architecture](#️-architecture)
- [Prerequisites](#-prerequisites)
- [Installation](#-installation)
- [Configuration](#️-configuration-options)
- [Usage](#-usage)
- [Docker Deployment](#-docker-deployment)
- [Troubleshooting](#-troubleshooting)
- [License](#-license)

---

## 🏗️ Architecture

Montscan is written entirely in Rust, with no external programs or C libraries required at runtime (PDF pages are rendered in-process). It is a single crate; all code lives in `src/`. Each ingress is a module under `src/ingress/`, built on an existing protocol library, and implements the `Ingress` trait. Ingresses only hand finished PDFs to a `DocumentProcessor`; they know nothing about AI naming or delivery.

| Module | Role | Built on |
|---|---|---|
| `ingress` | `Ingress` / `DocumentProcessor` traits | — |
| `ingress::ftp` | FTP ingress | [`libunftp`](https://crates.io/crates/libunftp) + [`unftp-sbe-fs`](https://crates.io/crates/unftp-sbe-fs) |
| `ingress::sftp` | SFTP ingress | [`russh`](https://crates.io/crates/russh) + [`russh-sftp`](https://crates.io/crates/russh-sftp) |
| `ingress::scp` | SCP ingress | [`russh`](https://crates.io/crates/russh) (no SCP server crate exists; the small SCP sink protocol is implemented in the module) |
| `ingress::smb` | SMB share poller | [`smb`](https://crates.io/crates/smb) (pure Rust SMB2/3 client) |
| `ingress::folder` | Local folder watcher | `tokio` |
| `ingress::ssh` | Host key, auth and path jailing shared by SFTP and SCP | `russh` |
| `agent` | PDF rendering and Ollama filename generation | [`hayro`](https://crates.io/crates/hayro) (pure Rust PDF renderer), `reqwest`, `image` |
| `egress` | WebDAV / Samba / local delivery | `reqwest`, `smb` |
| `config`, `env`, `banner` | Settings and startup banner | — |

---

## 🔧 Prerequisites

- **Rust 1.92+** (to build from source)
- **Ollama** - [Installation guide](https://ollama.ai/) with a vision model (e.g., `llava`, `llama3.2-vision`)
- **WebDAV server** or **SMB share** (optional) - For processed file delivery

---

## 📦 Installation

### Local Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/sh4den/Montscan.git
   cd Montscan
   ```

2. **Build the application**
   ```bash
   cargo build --release
   # binary: target/release/montscan
   ```

3. **Set up Ollama with a vision model**
   ```bash
   # Install Ollama from https://ollama.ai/
   ollama pull llava
   # or any other vision-capable model
   ```

---

## ⚙️ Configuration Options

At least one **ingress** must be enabled. At least one **egress provider** should be configured, or the folder watcher can act as both (rename/move in place). Egress is chosen in priority order: WebDAV, then Samba, then a local move when the folder watcher is enabled. With no egress at all, received files are left where they landed.

### Ingress — FTP Server

| Variable | Description | Default |
|---|---|---|
| `FTP_ENABLED` | Enable the FTP ingress server | `true` |
| `FTP_HOST` | FTP server bind address | `0.0.0.0` |
| `FTP_PORT` | FTP server port | `21` |
| `FTP_USERNAME` | FTP authentication username | `scanner` |
| `FTP_PASSWORD` | FTP authentication password | `scanner123` |
| `FTP_UPLOAD_DIR` | Local directory for received files | `./scans` |
| `FTP_PASSIVE_PORTS` | Passive data port range | `21000-21010` |
| `FTP_PASSIVE_HOST` | Address advertised for passive connections (set to the host's LAN IP behind Docker/NAT) | *(control connection address)* |

Files are processed once a `STOR` completes, or when an upload is renamed to `*.pdf`.

### Ingress — SFTP Server

| Variable | Description | Default |
|---|---|---|
| `SFTP_ENABLED` | Enable the SFTP ingress server | `false` |
| `SFTP_HOST` | Bind address | `0.0.0.0` |
| `SFTP_PORT` | Port | `2222` |
| `SFTP_USERNAME` | Username (password auth only; no shell access) | `scanner` |
| `SFTP_PASSWORD` | Password | `scanner123` |
| `SFTP_UPLOAD_DIR` | Directory clients see as `/` | `./scans` |
| `SFTP_HOST_KEY` | Ed25519 host key, generated on first start | `./ssh_host_ed25519_key` |

Files are processed when the client closes the file, or when an upload is renamed to `*.pdf`. Modern `scp` (OpenSSH 9+) uses the SFTP protocol and works against this port.

### Ingress — SCP Server

| Variable | Description | Default |
|---|---|---|
| `SCP_ENABLED` | Enable the SCP ingress server | `false` |
| `SCP_HOST` | Bind address | `0.0.0.0` |
| `SCP_PORT` | Port | `2223` |
| `SCP_USERNAME` | Username (password auth only; no shell access) | `scanner` |
| `SCP_PASSWORD` | Password | `scanner123` |
| `SCP_UPLOAD_DIR` | Directory clients see as `/` | `./scans` |
| `SCP_HOST_KEY` | Ed25519 host key, generated on first start (may be shared with SFTP) | `./ssh_host_ed25519_key` |

This speaks the legacy SCP protocol (`scp -t`), as used by scanners and `scp -O`. Uploads only; recursive (`-r`) uploads are supported.

### Ingress — Samba Server (polls a remote SMB share)

| Variable | Description | Default |
|---|---|---|
| `SAMBA_SERVER_ENABLED` | Enable the Samba ingress poller | `false` |
| `SAMBA_SERVER_HOST` | SMB server hostname or IP | `localhost` |
| `SAMBA_SERVER_PORT` | SMB port | `445` |
| `SAMBA_SERVER_USERNAME` | SMB username | - |
| `SAMBA_SERVER_PASSWORD` | SMB password | - |
| `SAMBA_SERVER_SHARE` | Share name to mount | `scans` |
| `SAMBA_SERVER_PATH` | Path within the share to watch | `/scans` |
| `SAMBA_SERVER_POLL_INTERVAL_SEC` | Polling interval in seconds | `10` |
| `SAMBA_SERVER_DELETE_AFTER_READ` | Remove file from share after download | `true` |
| `SAMBA_SERVER_WORK_DIR` | Local staging directory | *(FTP_UPLOAD_DIR)* |

### Ingress — Folder Watcher (monitors a local directory)

| Variable | Description | Default |
|---|---|---|
| `FOLDER_ENABLED` | Enable the local folder watcher | `false` |
| `FOLDER_INPUT_DIR` | Directory to watch for incoming PDFs | `./input` |
| `FOLDER_OUTPUT_DIR` | Destination for renamed files; omit to rename in place | - |
| `FOLDER_POLL_INTERVAL_SEC` | Polling interval in seconds | `5` |

### Egress — WebDAV Provider

| Variable | Description | Default |
|---|---|---|
| `WEBDAV_ENABLED` | Enable WebDAV upload | `false` |
| `WEBDAV_URL` | WebDAV server base URL | - |
| `WEBDAV_USERNAME` | WebDAV username | - |
| `WEBDAV_PASSWORD` | WebDAV password | - |
| `WEBDAV_UPLOAD_PATH` | Remote path for uploaded files | `/Documents/Scanned` |
| `WEBDAV_INSECURE` | Skip TLS verification | `false` |

### Egress — Samba Provider (uploads to an SMB share)

| Variable | Description | Default |
|---|---|---|
| `SAMBA_ENABLED` | Enable Samba upload | `false` |
| `SAMBA_HOST` | SMB server hostname or IP | `localhost` |
| `SAMBA_PORT` | SMB port | `445` |
| `SAMBA_USERNAME` | SMB username | - |
| `SAMBA_PASSWORD` | SMB password | - |
| `SAMBA_SHARE` | Destination share name | `scans` |
| `SAMBA_PATH` | Path within the share | `scans` |

### AI Processing

| Variable | Description | Default |
|---|---|---|
| `OLLAMA_HOST` | Ollama service URL | `http://localhost:11434` |
| `OLLAMA_MODEL` | Vision model to use | `qwen3-vl:8b` |
| `OLLAMA_NUM_CTX` | Context window (tokens) requested per call | `8192` |
| `OLLAMA_TIMEOUT` | Seconds to wait for Ollama before falling back to a timestamped name | `120` |
| `LANGUAGE` | Language for AI-generated filenames | `english` |

---

## 🚀 Usage

### Typical setups

**Scanner → FTP → WebDAV**
```bash
FTP_ENABLED=true
FTP_USERNAME=scanner
FTP_PASSWORD=secret
WEBDAV_ENABLED=true
WEBDAV_URL=https://cloud.example.com/remote.php/webdav
WEBDAV_USERNAME=user
WEBDAV_PASSWORD=secret
```

**Scanner → Samba share → local folder watcher → output folder**

This is the simplest setup when your scanner already drops files onto a Samba share that is mounted locally. Montscan watches the mount, renames each PDF with an AI-generated name, and moves it to an output directory.

```bash
FOLDER_ENABLED=true
FOLDER_INPUT_DIR=/mnt/scanner       # your Samba mount point
FOLDER_OUTPUT_DIR=/mnt/documents    # where renamed files land
FOLDER_POLL_INTERVAL_SEC=5
FTP_ENABLED=false
```

**Scanner → Samba share → local folder watcher → rename in place**

Same as above but without a separate output directory — files are renamed where they sit.

```bash
FOLDER_ENABLED=true
FOLDER_INPUT_DIR=/mnt/scanner
# FOLDER_OUTPUT_DIR not set → rename in place
FTP_ENABLED=false
```

### Running locally

```bash
cp .env.example .env
# edit .env with your values
export $(grep -v '^#' .env | xargs)
./target/release/montscan
```

Logging defaults to `info`; override with `RUST_LOG` (e.g. `RUST_LOG=debug`).

---

## 🐳 Docker Deployment

### Using Docker Compose

1. **Update environment variables in `docker-compose.yml`**

2. **(Optional) Enable GPU acceleration for Ollama**

   By default, Ollama runs on CPU. To use an NVIDIA GPU, set in `.env`:
   ```bash
   COMPOSE_PROFILES=gpu
   OLLAMA_GPU_COUNT=1
   ```
   Leave `COMPOSE_PROFILES=cpu` (or unset) for CPU-only.

3. **Start the container**

   By default the prebuilt image from GitHub Container Registry (`ghcr.io/sh4den/montscan`) is used:
   ```bash
   docker-compose up -d
   ```
   To build from source instead, uncomment the `build: .` line in `docker-compose.yml` and run `docker-compose up -d --build`.

4. **View logs**
   ```bash
   docker-compose logs -f
   ```

5. **Stop the container**
   ```bash
   docker-compose down
   ```

### Using Docker directly

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

---

## 🔍 Troubleshooting

### FTP Connection Refused
- Check that port 21 is not blocked by a firewall
- On Windows, allow the application through Windows Defender Firewall

### FTP Uploads Hang After Login (Docker)
- Passive data connections must reach the container: publish `FTP_PASSIVE_PORTS` and set `FTP_PASSIVE_HOST` to the Docker host's LAN IP

### SCP Fails with "subsystem request failed"
- OpenSSH 9+ `scp` uses the SFTP protocol by default. Use `scp -O` against the SCP port, or point the client at the SFTP port instead

### SSH Host Key Changed Warning
- The host key is generated on first start at `SFTP_HOST_KEY` / `SCP_HOST_KEY`. In Docker, keep it on the `./data` volume so it survives container recreation

### Folder Watcher Not Picking Up Files
- Verify `FOLDER_INPUT_DIR` points to the correct path inside the container (use volume mounts)
- Check that the directory is readable by the process running Montscan
- Increase `FOLDER_POLL_INTERVAL_SEC` if the scanner writes files slowly

### AI Naming Fails
- Verify Ollama is running: `ollama list`
- Ensure you have a vision-capable model installed (e.g., `llava`, `llama3.2-vision`)
- Montscan falls back to a timestamp filename (`scan_YYYYMMDD_HHMMSS.pdf`) on AI failure

### WebDAV Upload Fails
- Check credentials and URL
- Ensure the upload path exists on the server
- For Nextcloud: verify WebDAV is enabled and the URL ends with `/remote.php/webdav`
- Try `WEBDAV_INSECURE=true` if you use a self-signed certificate

### Scan Fails with "failed to parse PDF"
- Password-protected (encrypted) PDFs are not supported; disable PDF encryption on the scanner

---

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

## 🙏 Acknowledgments

- [libunftp](https://github.com/bolcom/libunftp) - Async FTP server library for Rust
- [russh](https://github.com/Eugeny/russh) and [russh-sftp](https://github.com/AspectUnk/russh-sftp) - SSH and SFTP server libraries for Rust
- [smb-rs](https://github.com/afiffon/smb-rs) - Pure Rust SMB2/3 client
- [Ollama](https://ollama.ai/) - Local AI vision model runner
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client used for Ollama and WebDAV

---

## 📧 Contact

For questions or support, please open an issue on GitHub.

---

<div align="center">
  <strong>Made with ❤️ for automated document management</strong>
</div>

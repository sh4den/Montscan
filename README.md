> [!NOTE] 
> Montscan is not fully production-ready and is currently in active development but is fairly usable.</br> 
> It achieve **97.5%** success rate on a small test set of 1000 documents.

<div align="center">
  <img width="256" height="256" alt="image" src="https://github.com/user-attachments/assets/ced68675-3338-4e7e-b9a0-5a9fc887aeac" />
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
- 📥 **Samba Server** - Polls a remote SMB share for incoming PDFs
- 📂 **Folder Watcher** - Monitors a local directory; renames in place or moves to an output folder
- 👁️ **Vision AI Processing** - Analyzes scanned documents using Ollama vision models
- 🤖 **AI-Powered Naming** - Generates descriptive, date-aware filenames via Ollama
- ☁️ **WebDAV Integration** - Uploads processed documents to Nextcloud, ownCloud, and any WebDAV server
- 🗂️ **Samba Provider** - Delivers processed documents to an SMB/CIFS network share
- 🐳 **Docker Support** - Easy deployment with Docker Compose

---

## 📋 Table of Contents

- [Prerequisites](#-prerequisites)
- [Installation](#-installation)
- [Configuration](#️-configuration-options)
- [Usage](#-usage)
- [Docker Deployment](#-docker-deployment)
- [Troubleshooting](#-troubleshooting)
- [License](#-license)

---

## 🔧 Prerequisites

- **Go 1.24+**
- **Poppler** (pdftoppm) or **ImageMagick** - For PDF to image conversion
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
   go build -o montscan .
   ```

3. **Install Poppler or ImageMagick**
    - **Windows**: Download from [GitHub Releases](https://github.com/oschwartz10612/poppler-windows/releases)
    - **Linux**: `sudo apt-get install poppler-utils`
    - **macOS**: `brew install poppler`

4. **Set up Ollama with a vision model**
   ```bash
   # Install Ollama from https://ollama.ai/
   ollama pull llava
   # or any other vision-capable model
   ```

---

## ⚙️ Configuration Options

At least one **ingress** must be enabled. At least one **egress provider** should be configured, or the folder watcher can act as both (rename/move in place).

### Ingress — FTP Server

| Variable | Description | Default |
|---|---|---|
| `FTP_ENABLED` | Enable the FTP ingress server | `true` |
| `FTP_HOST` | FTP server bind address | `0.0.0.0` |
| `FTP_PORT` | FTP server port | `21` |
| `FTP_USERNAME` | FTP authentication username | `scanner` |
| `FTP_PASSWORD` | FTP authentication password | `scanner123` |
| `FTP_UPLOAD_DIR` | Local directory for received files | `./scans` |

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
| `OLLAMA_MODEL` | Vision model to use | `llava` |
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
./montscan
```

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

3. **Build and start the container**
   ```bash
   docker-compose up -d
   ```

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

### Poppler/ImageMagick Not Found
- Install Poppler (`pdftoppm`) or ImageMagick and ensure the binary is in `PATH`
- Windows: add Poppler's `bin` folder to the `PATH` environment variable

---

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

## 🙏 Acknowledgments

- [goftp/server](https://github.com/goftp/server) - Go FTP server library
- [go-smb2](https://github.com/hirochachacha/go-smb2) - SMB2 protocol client for Go
- [Ollama](https://ollama.ai/) - Local AI vision model runner
- [gowebdav](https://github.com/studio-b12/gowebdav) - WebDAV client for Go
- [fatih/color](https://github.com/fatih/color) - Colorful terminal output

---

## 📧 Contact

For questions or support, please open an issue on GitHub.

---

<div align="center">
  <strong>Made with ❤️ for automated document management</strong>
</div>

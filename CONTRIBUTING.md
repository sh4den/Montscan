# Contributing to Montscan

Thanks for taking the time to help. Bug reports, fixes, new ingresses and egresses, and documentation improvements are all welcome.

## Reporting bugs

Open an issue and include:

- The Montscan version (the release tag or Docker image tag, or the commit if you built from source)
- Which ingress and egress you use (FTP, SFTP, SCP, Samba, folder watcher / WebDAV, Samba, local)
- Your scanner model if the problem is about receiving files
- The Ollama model if the problem is about naming
- Logs from a run with `RUST_LOG=debug`

Remove passwords, hostnames and anything else private from logs and config before posting them.

If a specific PDF triggers the problem and you can share it (or a similar one without personal data), attach it. That's usually the fastest way to get it fixed.

## Suggesting features

Open an issue describing what you're trying to do and why the current options don't cover it. For anything larger than a small change, please open the issue before writing code so we can agree on the approach first.

## Development setup

You need Rust 1.92 or newer and, to test naming end to end, a running [Ollama](https://ollama.ai/) with a vision model.

```bash
git clone https://github.com/sh4den/Montscan.git
cd Montscan
cargo build
cp .env.example .env    # then edit it
```

To run it locally:

```bash
export $(grep -v '^#' .env | xargs)
RUST_LOG=debug cargo run
```

Ports below 1024 (like the default FTP port 21) need extra privileges on Linux. During development it's easier to set `FTP_PORT=2121`.

The unit tests don't need Ollama or any network services.

## Before opening a pull request

Make sure these pass:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
```

Clippy should report no warnings. If you change behaviour, add or update tests for it. If you add or change a setting, update `.env.example`, `docker-compose.yml` if relevant, and the configuration tables in the README.

## Code layout

- `src/ingress/` holds one module per way of receiving files. Each implements the `Ingress` trait from `src/ingress/mod.rs` and hands finished PDFs to the `DocumentProcessor` by calling `ingress::dispatch`. An ingress should not know anything about naming or delivery.
- `src/agent/` renders the first page of a PDF and asks Ollama for a filename.
- `src/egress/` delivers the renamed file (WebDAV, Samba, or a local move).
- `src/config.rs` and `src/env.rs` read settings from environment variables.

To add a new ingress, create a module under `src/ingress/`, add its settings to the config, and start it from `main.rs` alongside the others. The existing FTP and folder watcher modules are the simplest examples to follow.

Runtime dependencies should stay pure Rust. Montscan deliberately doesn't shell out to external programs or link against C libraries, so the binary and Docker image stay self-contained.

## Commits

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(ingress): add SFTP server
fix(ftp): process uploads renamed to .pdf
docs(readme): document OLLAMA_TIMEOUT
```

Common types are `feat`, `fix`, `refactor`, `test`, `docs`, `build`, `ci` and `chore`. The scope is usually the module you touched. The release notes are generated from these prefixes, so please stick to them.

Keep each commit focused on one change. It's fine to have several commits in a pull request.

## Pull requests

- Branch off `dev` and open your pull request against `dev`, not `main`. `main` only receives release merges.
- Describe what the change does and how you tested it. For ingress changes, mention which scanner or client you tested with.
- Keep unrelated changes (formatting sweeps, dependency bumps) out of feature pull requests.

## Releases

Releases are handled by the maintainer. When `dev` is ready, a pull request titled `Update vX.Y.Z` is opened from `dev` to `main`. Merging it creates the tag and GitHub release, and publishes the Docker image.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE), the same as the rest of the project.

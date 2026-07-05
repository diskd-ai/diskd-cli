# diskd CLI

`diskd` is a small Rust command-line client for the diskd Drive API. It gives
humans, shell scripts, and coding agents a Unix-style interface for listing,
reading, searching, uploading, syncing, and querying Drive files through the
public `apis-service` gateway.

The CLI is published as the public `diskd-ai/diskd-cli` GitHub repository.
Release tags build platform archives and SHA-256 checksum files for Linux,
macOS, and Windows.

## What You Can Do

- Browse Drive paths with `ls`, `tree`, `glob`, and `stat`.
- Stream file bytes with `cat`.
- Search indexed content with `grep` and `vsearch`.
- Ask natural-language questions over CSV, TSV, XLS, and XLSX files with `biquery`.
- Work with generic Drive DBs through `database` (alias `db`).
- Create, insert, query, commit, inspect, and drop Telegram Drive DBs with `telegram-db`.
- Upload files, create folders, rename, copy, remove, and one-way sync folders.
- Run an embedded MCP stdio server with Drive tools for MCP clients.
- Update itself from signed GitHub release checksums.
- Use text output for humans or `--json` output for scripts.

## Install

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh
```

Pin a release:

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_VERSION=v0.1.5 sh
```

Install into a custom directory:

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_INSTALL_DIR="$HOME/bin" sh
```

The installer detects your platform, downloads the matching archive from GitHub
Releases, verifies the `.sha256` file, and installs `diskd`.

Update an installed binary:

```sh
diskd update
```

Normal human-facing commands check GitHub Releases on startup. When a newer
release is available, `diskd` prints a yellow stderr notice with the update
command. Checks are skipped for `--json`, `--quiet`, and MCP stdio serving.

## Quick Start

Authenticate through the browser:

```sh
diskd login
```

This opens `https://app.iosya.com/oauth-apps`, creates diskd CLI credentials
from your logged-in browser session, and stores the returned bearer token.

Use the development app host:

```sh
diskd login --dev
```

Authenticate with an already-issued bearer token:

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

Or keep a token only in the environment:

```sh
export APIS_ACCESS_TOKEN="..."
export APIS_BASE_URL="https://apis.iosya.com"
```

Check identity metadata decoded from the token:

```sh
diskd --json whoami
```

List projects and select one as the current context:

```sh
diskd --json set-context --list
diskd set-context "Project Name"
diskd get-context
```

Work with files under that project:

```sh
diskd mkdir docs
diskd upload ./report.pdf --dest docs --force
diskd ls docs
diskd cat docs/report.pdf > report.pdf
diskd grep "payment terms" docs --limit 10 --offset 0
diskd vsearch "contract renewal clauses" docs/report.pdf --limit 5 --offset 0
```

Global flags must be placed before the subcommand:

```sh
diskd --json ls docs
```

## Common Commands

```sh
diskd ls [path] [--recursive] [--long]    # <DIR>/<FILE>, size, index status, name + display metadata
diskd tree [path] -L 2 -s                 # recursive Drive tree, depth-limited
diskd glob "**/*.pdf" [--path docs]
diskd grep "exact text" [path...] --limit 10 --offset 0
diskd vsearch "semantic query" [path...] --limit 10 --offset 0
diskd cat path/to/file > local-file
diskd read path/to/file --limit 3 --offset 0
diskd stat path/to/file
diskd biquery "what is the total amount?" sheet.csv
diskd --json database query generic-db "SELECT id, text FROM messages LIMIT 20" --db-type telegram
diskd --json telegram-db query team-chat "SELECT id, text FROM messages LIMIT 20"
diskd upload ./file.txt --dest docs --force
diskd sync ./local-folder --dest docs --once
diskd update
diskd mcp serve
```

See [docs/commands.md](docs/commands.md) for the full command reference.

## Documentation

- [Quick start](docs/quick-start.md)
- [Command reference](docs/commands.md)
- [How-tos](docs/how-to.md)
- [Configuration and authentication](docs/configuration.md)
- [MCP server](docs/mcp.md)
- [Development and releases](docs/development.md)

## Configuration

By default, `diskd` uses `$HOME/.diskd/`:

```text
$HOME/.diskd/
  config.yaml
  credentials
```

You can override the state directory with `DISKD_HOME`.

Important environment variables:

```sh
export APIS_BASE_URL="https://apis.iosya.com"
export APIS_ACCESS_TOKEN="..."
export DISKD_HOME="$HOME/.diskd"
export DISKD_NO_UPDATE_CHECK=1
```

Resolution order is:

```text
CLI flags -> environment -> config files -> built-in defaults
```

## MCP

Run the embedded MCP stdio server:

```sh
diskd mcp serve
```

When launched directly in a terminal, this prints an LLM-agent MCP
configuration snippet. When launched by an MCP client over stdio, it starts the
server protocol without human text on stdout.

The server exposes these tool names:

```text
tools__ls
tools__read
tools__glob
tools__grep
tools__vsearch
tools__bi_query
```

See [docs/mcp.md](docs/mcp.md) for client configuration examples.

## Development

```sh
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Release builds are produced by `.github/workflows/release.yml` when a `v*` tag
is pushed or when the workflow is run manually.

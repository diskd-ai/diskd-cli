---
name: diskd-cli
description: diskd CLI (`diskd`) usage for the diskd Drive API through the public apis-service gateway. Use when listing, rendering trees, reading, searching, uploading, syncing, copying, moving, or deleting Drive files from the command line; running exact/BM25 search (`grep`), semantic search (`vsearch`), or natural-language (plain-English) questions over indexed CSV/TSV/XLS/XLSX spreadsheets where the Drive backend generates the SQL (`biquery`); creating, inserting, querying, committing, rolling back, inspecting, dropping, resolving, or setting status on generic Drive DBs (`database`, alias `db`) and Telegram Drive DBs (`telegram-db`); managing auth (`login`/`logout`/`whoami`), project context (`set-context`/`get-context`), self-update (`update`), JSON output for scripts (`--json`), or the embedded MCP stdio server (`diskd mcp serve`). Triggers on mentions of diskd, diskd CLI, `diskd ls/tree/cat/read/grep/vsearch/biquery/database/db/telegram-db/upload/sync`, the diskd drive, or adding diskd as an MCP server to an agent.
---

# diskd CLI

`diskd` is a Rust command-line client for the diskd Drive API. It gives humans,
shell scripts, and coding agents a Unix-style interface for listing, reading,
searching, uploading, syncing, and querying Drive files through the public
`apis-service` gateway. Output is human text by default and machine-readable
JSON with `--json`.

## Command Shape

```sh
diskd [GLOBAL FLAGS] <command> [ARGS] [FLAGS]
```

Global flags MUST be placed **before** the subcommand. `diskd --json ls docs`
works; `diskd ls docs --json` does not.

| Global flag | Purpose |
| --- | --- |
| `--json` | Print machine-readable JSON where supported. |
| `-q`, `--quiet` | Reduce progress/status messages on stderr; skips the startup update check. |
| `--base-url <url>` | Override the gateway base URL for one command. |
| `-p`, `--project <id>` | Override the current project for one command. |
| `-w`, `--workspace <id>` | Reserved compatibility flag; workspace scope comes from the token, not this flag. |
| `--config <path>` | Use a custom config file instead of `$DISKD_HOME/config.yaml`. |

## Install

```sh
# Latest release (detects platform, verifies .sha256, installs `diskd`)
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh

# Pin a version
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_VERSION=v0.1.5 sh

# Custom directory
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_INSTALL_DIR="$HOME/bin" sh
```

Update an installed binary with `diskd update` (`--force` to reinstall the
latest even when versions match). Human-facing commands print a yellow stderr
update notice when a newer release exists; the check is skipped for `--json`,
`--quiet`, and `diskd mcp serve`, and disabled by `DISKD_NO_UPDATE_CHECK=1`.

## Quick Start: auth -> context -> files

```sh
# 1. Authenticate (browser login stores only the bearer token under $DISKD_HOME/credentials)
diskd login                                       # use --dev for https://app.upgraide.dev/oauth-apps
diskd login --token "$APIS_ACCESS_TOKEN"          # non-interactive token login

# 2. Verify identity (decodes workspace/subject from the token; never prints it)
diskd --json whoami

# 3. Pick a project as the current path context
diskd --json set-context --list                   # list accessible projects
diskd set-context "Project Name"                  # match by name or id
diskd get-context                                 # show current context
diskd set-context --root                           # clear project, use workspace root

# 4. Work with files under that context
diskd mkdir docs
diskd upload ./report.pdf --dest docs --force
diskd ls docs
diskd cat docs/report.pdf > report.pdf
diskd --json grep "payment terms" docs
diskd --json vsearch "contract renewal clauses" docs/report.pdf --top 5
```

## Command Surface at a Glance

| Command | Purpose |
| --- | --- |
| `ls [path]` | List a Drive path as `<DIR>/<FILE>`, size, indexing status, and copyable name with display metadata. Flags: `--recursive`, `--long`, `--show-hidden`, `--show-system`. |
| `tree [path]` | Render a recursive Drive tree. Flags: `-L`/`--depth`/`--deep <n>`, `-a`/`--all`, `-d`/`--dirs-only`, `-f`/`--full-path`, `-s`/`--size`, `--show-system`. |
| `glob <pattern>` | Glob match. Flags: `--path <dir>`, `--show-hidden`, `--show-system`. |
| `grep <query> [paths...]` | Exact/BM25 content search. Flags: `--limit`, `--offset`. Paths default to the context root. |
| `vsearch <query> [paths...]` | Semantic search. Flags: `--limit` (alias `--top`), `--offset`. |
| `cat <path>` | Stream raw file bytes to stdout. Flag: `--version <n>`. |
| `read <path>` | Structured indexed document parts. Flags: `--limit`/`--offset` aliases for `--parts-limit`/`--parts-offset`. |
| `stat <path>` | Path metadata. |
| `biquery <question> [paths...]` | Natural-language query over indexed CSV/TSV/XLS/XLSX/mailbox spreadsheets; the backend converts the question to SQL and runs it. |
| `database <subcommand>` (`db`) | Generic Drive DB lifecycle. Subcommands: `create`, `insert`, `query`, `commit`, `rollback`, `metadata`, `drop`, `set-status`, `resolve-by-inode`, `resolve-with-settings`. |
| `telegram-db <subcommand>` | Telegram Drive DB lifecycle. Subcommands: `create`, `insert`, `query`, `commit`, `metadata`, `drop`. |
| `upload <local...>` | Upload file(s)/folder(s). Flags: `--dest <dir>`, `--recursive`, `--force`. |
| `mkdir <path>` | Create a folder. |
| `rm <path>` | Delete. Flag: `--recursive`. |
| `mv <src> <dst>` | Rename/move. |
| `cp <src> <dst>` | Copy (download then upload). Flag: `--force`. |
| `sync <folder>` | One-way local -> Drive. Flags: `--dest`, `--once`, `--interval-seconds <n>` (default 2). |
| `login` | Store a token or exchange client credentials. Flags: `--token`, `--credentials-file`. |
| `logout` | Delete stored credentials. |
| `whoami` | Decode token identity metadata. |
| `set-context` | Select project context. Flags: `--list`, `--root` (alias `--clear`). |
| `get-context` | Print current context. |
| `version` | Print CLI version. |
| `update` | Self-update from GitHub releases. Flag: `--force`. |
| `mcp serve` | Run the embedded MCP stdio server. |

Full flag-by-flag detail and the exact Drive API method each command calls are
in [references/commands.md](references/commands.md).

## Path Rules

Paths are resolved relative to the current context. The CLI rejects `.` and
`..` segments **before** any network call.

```text
# No project context (workspace root)
docs/a.txt   -> /docs/a.txt

# Project 01PROJECT selected
docs/a.txt   -> /Projects/01PROJECT/docs/a.txt
```

The selected project is local CLI context only; it is never sent as a public
Drive API field. See [references/auth-and-config.md](references/auth-and-config.md).

## Key Behaviors to Know

- **`--json` before the command** for scripting; pipe into `jq`:
  `diskd --json set-context --list | jq -r '.[].name'`.
- **`vsearch` reliability**: a specific **file** path is more reliable than a
  directory path, because the backend may not expand directory inodes for
  vector search. If a directory vsearch fails with a "directory without file id"
  style error, retry against a file or use `grep`.
- **`grep` flags `--ignore-case` / `--files-with-matches` are parsed but
  rejected** -- the current Drive grep contract has no matching fields. Do not
  rely on them.
- **`cat` writes bytes to stdout**, so redirection and pipes work as expected.
- **`biquery` takes a plain-language question, not SQL.** The Drive backend
  reads the spreadsheet schema and uses an LLM to generate and run the SQL,
  returning a result table, e.g.
  `diskd --json biquery "total amount grouped by name" data/table.csv`. Point it
  at indexed spreadsheet files (`.csv`, `.tsv`, `.xls`, `.xlsx`, `.mailbox`); a
  directory path is expanded to the spreadsheet files inside.
- **`database query` is the generic SQL path.** It calls `drive/db/query`.
  Use `--db-type telegram`/`webarchive`/`session` when needed to disambiguate
  typed DB names. `database` also exposes commit, rollback, metadata, drop,
  status, and inode resolution methods.
- **`telegram-db query` is the SQL path.** It calls `drive/telegram/query`
  against a named Telegram SQLite DB. Use `--parameters '[...]'` for positional
  SQL parameters. `telegram-db insert` requires a JSON array through `--rows` or
  `--rows-file`.
- **`upload` preserves paths relative to each provided local directory** and
  computes SHA-256 per file (start -> PUT bytes -> commit).

## References

- **Full command reference** (every flag + Drive method): [references/commands.md](references/commands.md)
- **Auth, config, env vars, path context**: [references/auth-and-config.md](references/auth-and-config.md)
- **MCP stdio server** (config, tool names, JSON-RPC smoke tests): [references/mcp.md](references/mcp.md)
- **End-to-end workflows** (CSV/BI, CI, sync, agent integration, troubleshooting): [references/workflows.md](references/workflows.md)

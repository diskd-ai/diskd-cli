# Command Reference

`diskd` commands follow this shape:

```sh
diskd [GLOBAL FLAGS] <command> [ARGS] [FLAGS]
```

Global flags must be placed before the subcommand.

## Global Flags

| Flag | Description |
| --- | --- |
| `--base-url <url>` | Override the API gateway base URL. |
| `--json` | Print machine-readable JSON when supported. |
| `--quiet`, `-q` | Reduce progress and status messages on stderr. |
| `--config <path>` | Use a custom config file instead of `$DISKD_HOME/config.yaml`. |
| `-p`, `--project <id>` | Override the current project for one command. |
| `-w`, `--workspace <id>` | Reserved compatibility flag; workspace scope comes from the token. |

## Command Parameters

| Command | Positional args | Command flags | API/behavior | Output notes |
| --- | --- | --- | --- | --- |
| `ls` | `[path]` default context root | `--recursive`, `--long`, `--show-hidden`, `--show-system` | `paths/tools/ls` | Text prints `displayName`/`display_name` first; `--json` preserves raw entries. |
| `glob` | `<pattern>` | `--path <dir>`, `--show-hidden`, `--show-system` | `paths/tools/glob` | Matching path entries. |
| `grep` | `<query> [paths...]` | `--limit <n>`, `--offset <n>` | `paths/tools/grep` | Exact/BM25 search; omitted paths use context root. |
| `vsearch` | `<query> [paths...]` | `--limit <n>`, `--top <n>` alias, `--offset <n>` | `paths/tools/vsearch` | Semantic search; prefer file paths when possible. |
| `cat` | `<path>` | `--version <n>` | `drive/files/download-url` plus byte download | Raw bytes to stdout. |
| `read` | `<path>` | `--limit`/`--parts-limit`, `--offset`/`--parts-offset` | `paths/tools/read` | Structured parts plus pagination metadata. |
| `stat` | `<path>` | none | `paths/tools/inode-ls` | Path metadata. |
| `biquery` | `<question> [paths...]` | none | `paths/tools/bi-query` | Natural-language spreadsheet question, not SQL. |
| `database` (`db`) | subcommand-specific | `create`, `insert`, `query`, `commit`, `rollback`, `metadata`, `drop`, `set-status`, `resolve-by-inode`, `resolve-with-settings` | `drive/db/*` | Generic Drive DB working API with optional `--db-type`. |
| `telegram-db` | subcommand-specific | `create`, `insert`, `query`, `commit`, `metadata`, `drop` flags | `drive/telegram/*` | Telegram SQLite DB working API; `query` uses SQL against the named DB. |
| `upload` | `<local...>` | `--dest <dir>`, `--recursive`, `--force` | upload start, PUT, commit | Uploads files/folders. |
| `mkdir` | `<path>` | none | `drive/paths/create` | Creates folder. |
| `rm` | `<path>` | `--recursive` | `drive/paths/delete` | Deletes file/folder. |
| `mv` | `<src> <dst>` | none | `drive/paths/rename` | Move/rename. |
| `cp` | `<src> <dst>` | `--force` | client download then upload | Copy through local client. |
| `sync` | `<folder>` | `--dest <dir>`, `--once`, `--interval-seconds <n>` | repeated upload passes | One-way local-to-Drive. |
| `login` | none | `--dev`, `--app-url`, `--token`, `--credentials-file` | browser login or token exchange | Stores bearer token. |
| `logout` | none | none | local credential delete | Clears auth. |
| `whoami` | none | none | local JWT decode | Prints workspace/subject metadata. |
| `set-context` | `[project]` | `--list`, `--root`/`--clear` | project REST list or local config write | Selects local project path prefix. |
| `get-context` | none | none | local config read | Prints active context. |
| `version` | none | none | local metadata | Prints CLI version. |
| `update` | none | `--force` | GitHub release update | Replaces binary after checksum verification. |
| `mcp serve` | none | none | embedded MCP stdio | Prints agent config when run directly. |

## Auth and Meta

### `login`

Open browser login:

```sh
diskd login
diskd login --dev
```

By default, `diskd login` opens `https://app.iosya.com/oauth-apps`. The `--dev`
flag uses `https://app.upgraide.dev/oauth-apps`.

Store an existing token:

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

Request a token with OAuth client credentials:

```sh
diskd login --credentials-file ./credentials.json
```

The client first requests gateway scopes used by Drive and project commands. If
the issuer rejects those scopes, the CLI retries with the client's default
scope set.

### `logout`

```sh
diskd logout
```

Deletes the stored credential file.

### `whoami`

```sh
diskd --json whoami
```

Decodes workspace and subject metadata from the current bearer token.

### `version`

```sh
diskd version
diskd --json version
```

### `update`

```sh
diskd update
diskd update --force
```

Checks the latest `diskd-ai/diskd-cli` GitHub release, downloads the matching
platform archive and `.sha256` file, verifies the checksum, and replaces the
running binary. `--force` reinstalls the latest release even when the compiled
version matches.

Most human-facing commands perform a short startup update check. If a newer
release exists, `diskd` prints a yellow stderr notice:

```text
diskd 0.1.4 is available; current is 0.1.3. Run `diskd update`.
```

Startup checks are skipped for `--json`, `--quiet`, and `diskd mcp serve`.

## Project Context

### `set-context --list`

```sh
diskd --json set-context --list
```

Calls `GET /v1/platform/projects/api/projects` and prints project `id` and
`name`.

### `set-context <project>`

```sh
diskd set-context "Project Name"
diskd set-context 01PROJECTID
```

Stores the selected project in `config.yaml`. The project may be matched by id
or name.

### `set-context --root`

```sh
diskd set-context --root
```

Clears project context and uses the workspace root.

### `get-context`

```sh
diskd --json get-context
```

Prints the stored project context or the workspace root default.

## Path Rules

With no project context:

```text
docs/a.txt -> /docs/a.txt
/docs/a.txt -> /docs/a.txt
```

With project `01PROJECT`:

```text
docs/a.txt -> /Projects/01PROJECT/docs/a.txt
/docs/a.txt -> /Projects/01PROJECT/docs/a.txt
```

The CLI rejects `.` and `..` path segments before making a network request.

## Read and Query Commands

### `ls`

```sh
diskd ls [path] [--recursive] [--long] [--show-hidden] [--show-system]
```

Calls `paths/tools/ls`.

### `glob`

```sh
diskd glob "**/*.pdf" --path docs
```

Calls `paths/tools/glob`.

### `grep`

```sh
diskd --json grep "payment terms" docs contracts --limit 20 --offset 0
```

Calls `paths/tools/grep`. Omitted paths default to the current context root.
`--limit <n>` and `--offset <n>` page matched documents.

`--ignore-case` and `--files-with-matches` are parsed but rejected because the
current Drive grep contract has no matching fields.

### `vsearch`

```sh
diskd --json vsearch "renewal clauses" docs/agreement.pdf --limit 5 --offset 0
```

Calls `paths/tools/vsearch`. `--limit <n>` (alias `--top <n>`) and
`--offset <n>` page matched documents. Omitted paths default to the current
context root.

### `cat`

```sh
diskd cat docs/report.pdf > report.pdf
```

Calls `drive/files/download-url`, then downloads and streams the returned URL to
stdout.

### `read`

```sh
diskd --json read docs/report.pdf --limit 5 --offset 0
```

Calls `paths/tools/read` and returns structured indexed document parts.
`--limit`/`--offset` are aliases for `--parts-limit`/`--parts-offset`.

### `stat`

```sh
diskd --json stat docs/report.pdf
```

Calls `paths/tools/inode-ls`, the deployed path-based metadata surface.

### `biquery`

```sh
diskd --json biquery "what is the total amount?" docs/table.csv
diskd --json biquery "total amount grouped by name" docs/table.csv
```

Calls `paths/tools/bi-query` for indexed CSV, TSV, XLS, and XLSX files. The
query is a natural-language question; Drive generates and runs the SQL.

### `database` / `db`

```sh
diskd --json database create generic-db \
  --schema '{"items":["CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"]}'
diskd --json db insert generic-db messages --rows '[{"id":1,"text":"hello"}]'
diskd --json database query generic-db "SELECT id, text FROM messages LIMIT 20"
diskd --json database query generic-db "SELECT id FROM messages WHERE text = ?" --parameters '["hello"]'
diskd --json database commit generic-db
diskd --json database rollback generic-db
diskd --json database metadata generic-db
diskd --json database drop generic-db
diskd --json database set-status generic-db ready --error "optional diagnostic"
diskd --json database resolve-by-inode db_inode_value
diskd --json database resolve-with-settings db_inode_value --db-type telegram
```

Drive methods:

```text
create                -> drive/db/create
insert                -> drive/db/insert
query                 -> drive/db/query
commit                -> drive/db/commit
rollback              -> drive/db/rollback
metadata              -> drive/db/metadata
drop                  -> drive/db/drop
set-status            -> drive/db/set-status
resolve-by-inode      -> drive/db/resolve-by-inode
resolve-with-settings -> drive/db/resolve-with-settings
```

All `database` operations that address a DB by name accept optional
`--db-type <database|mailbox|telegram|webarchive|session>`. `create --schema`/
`--schema-file` must be a JSON object. `insert --rows`/`--rows-file` must be a
JSON array of row objects. `query --parameters`/`--parameters-file` must be a
JSON array for positional SQL parameters.

### `telegram-db`

```sh
diskd --json telegram-db create team-chat \
  --schema '{"items":["CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"]}'
diskd --json telegram-db insert team-chat messages --rows '[{"id":1,"text":"hello"}]'
diskd --json telegram-db query team-chat "SELECT id, text FROM messages LIMIT 20"
diskd --json telegram-db query team-chat "SELECT id FROM messages WHERE text = ?" --parameters '["hello"]'
diskd --json telegram-db commit team-chat
diskd --json telegram-db metadata team-chat
diskd --json telegram-db drop team-chat
```

Drive methods:

```text
create   -> drive/telegram/create
insert   -> drive/telegram/insert
query    -> drive/telegram/query
commit   -> drive/telegram/commit
metadata -> drive/telegram/metadata
drop     -> drive/telegram/drop
```

Telegram DB names may be provided with or without `.telegram`; the Drive
handler appends the extension. `create --schema`/`--schema-file` must be a JSON
object with an `items` array of SQL statements. `insert --rows`/`--rows-file`
must be a JSON array of row objects.
`query --parameters`/`--parameters-file` must be a JSON array for positional SQL
parameters.

## Write and Manage Commands

### `upload`

```sh
diskd upload ./file.txt --dest docs --force
diskd upload ./folder --dest docs --recursive --force
```

For each file, the CLI computes SHA-256, calls `drive/upload/start`, PUTs bytes
to the returned upload URL, and calls `drive/upload/commit`.

### `mkdir`

```sh
diskd mkdir docs
```

Calls `drive/paths/create`.

### `rm`

```sh
diskd rm docs/file.txt
diskd rm docs --recursive
```

Calls `drive/paths/delete`.

### `mv`

```sh
diskd mv docs/a.txt docs/b.txt
```

Calls `drive/paths/rename`.

### `cp`

```sh
diskd cp docs/a.txt docs/copy.txt --force
```

Downloads the source file and uploads it to the destination.

### `sync`

```sh
diskd sync ./local-folder --dest docs --once
diskd sync ./local-folder --dest docs --interval-seconds 5
```

`sync` is one-way local-to-Drive. With `--once`, it uploads the current tree and
exits. Without `--once`, it repeats the upload pass on a polling interval.

## MCP

```sh
diskd mcp serve
```

When run directly in a terminal, prints instructions for adding the server to
an LLM agent. When launched by an MCP client over stdio, starts the embedded
stdio MCP server. See [mcp.md](mcp.md).

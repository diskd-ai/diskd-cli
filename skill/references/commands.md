# diskd Command Reference

Complete flag-by-flag reference. Global flags always precede the subcommand.

## Contents

- [Global flags](#global-flags)
- [Read and query](#read-and-query): `ls`, `tree`, `glob`, `grep`, `vsearch`, `cat`, `read`, `stat`, `biquery`, `database`, `telegram-db`
- [Write and manage](#write-and-manage): `upload`, `mkdir`, `rm`, `mv`, `cp`, `sync`
- [Auth and meta](#auth-and-meta): `login`, `logout`, `whoami`, `version`, `update`
- [Project context](#project-context): `set-context`, `get-context`
- [MCP](#mcp): `mcp serve`

## Global flags

Placed before the subcommand.

| Flag | Description |
| --- | --- |
| `--base-url <url>` | Override the API gateway base URL. |
| `--json` | Print machine-readable JSON when supported. |
| `--quiet`, `-q` | Reduce progress/status on stderr; also skips the startup update check. |
| `--config <path>` | Use a custom config file instead of `$DISKD_HOME/config.yaml`. |
| `-p`, `--project <id>` | Override the current project for one command. |
| `-w`, `--workspace <id>` | Reserved compatibility flag; workspace scope always comes from the token. |

## Agent command matrix

Use this table when calling `diskd` from an agent skill. Put global flags before
the subcommand, and put command flags after the subcommand.

| Command | Positional args | Command flags | API/behavior | Output notes |
| --- | --- | --- | --- | --- |
| `ls` | `[path]` default context root | `--recursive`, `--long`, `--show-hidden`, `--show-system` | `paths/tools/ls` with `path`, optional booleans | Text is ls-like: type marker (`<DIR>`, `<FILE>`), size, indexing status, then copyable Drive name with display metadata in parentheses. `--json` preserves raw entries. |
| `tree` | `[path]` default context root | `-L`/`--depth <n>` (`--deep` alias), `-a`/`--all`, `-d`/`--dirs-only`, `-f`/`--full-path`, `-s`/`--size`, `--show-system` | recursive `paths/tools/ls` with `path`, `recursive=true`, optional visibility booleans | ASCII tree over Drive entries. `--json` preserves raw recursive ls response. |
| `glob` | `<pattern>` | `--path <dir>`, `--show-hidden`, `--show-system` | `paths/tools/glob` with `pattern`, optional `path` | Returns matching path entries. |
| `grep` | `<query> [paths...]` | `--limit <n>`, `--offset <n>`, unsupported parser-only `--ignore-case`, `--files-with-matches` | `paths/tools/grep` with `query`, normalized `paths`, optional `limit`, `offset` | Exact/BM25 indexed document search. Omitted paths search the context root. |
| `vsearch` | `<query> [paths...]` | `--limit <n>`, `--top <n>` alias, `--offset <n>` | `paths/tools/vsearch` with `query`, normalized `paths`, optional `limit`, `offset` | Semantic search. Prefer file paths when directory vector search is unreliable. |
| `cat` | `<path>` | `--version <n>` | `drive/files/download-url`, then authenticated byte download | Writes raw bytes to stdout; redirect for binary files. |
| `read` | `<path>` | `--limit <n>`/`--parts-limit <n>`, `--offset <n>`/`--parts-offset <n>` | `paths/tools/read` with `path`, optional `parts_limit`, `parts_offset` | Returns structured parts plus `total_parts`, `next_offset`, `eof`. |
| `stat` | `<path>` | none | `paths/tools/inode-ls` | Returns path metadata. |
| `biquery` | `<question> [paths...]` | none | `paths/tools/bi-query` with natural-language `query`, normalized `paths` | Question is not SQL. Use for indexed `.csv`, `.tsv`, `.xls`, `.xlsx`, `.mailbox`. |
| `database create` (`db create`) | `<name>` | `--schema <json>`, `--schema-file <path>`, `--check-exists`, `--recreate`, `--directory <dir>`, `--db-type <type>` | `drive/db/create` | Creates a generic Drive DB. |
| `database insert` | `<name> <table>` | `--rows <json-array>` or `--rows-file <path>`, `--db-type <type>` | `drive/db/insert` | Inserts row objects into a DB table. |
| `database query` | `<name> <sql>` | `--parameters <json-array>` or `--parameters-file <path>`, `--db-type <type>` | `drive/db/query` | Runs SQL against the named Drive DB. |
| `database commit` | `<name>` | `--db-type <type>` | `drive/db/commit` | Commits pending DB changes. |
| `database rollback` | `<name>` | `--db-type <type>` | `drive/db/rollback` | Rolls back pending DB changes. |
| `database metadata` | `<name>` | `--db-type <type>` | `drive/db/metadata` | Returns DB metadata. |
| `database drop` | `<name>` | `--db-type <type>` | `drive/db/drop` | Deletes the DB. |
| `database set-status` | `<name> <status>` | `--error <message>`, `--db-type <type>` | `drive/db/set-status` | Sets processor-visible status on a DB file. |
| `database resolve-by-inode` | `<db_inode>` | `--db-type <type>` | `drive/db/resolve-by-inode` | Resolves DB name/file id/status from inode. |
| `database resolve-with-settings` | `<db_inode>` | `--db-type <type>` | `drive/db/resolve-with-settings` | Resolves DB metadata and reads `settings`. |
| `telegram-db create` | `<name>` | `--schema <json>`, `--schema-file <path>`, `--recreate`, `--directory <dir>` | `drive/telegram/create` | Creates a Telegram DB; handler appends `.telegram` when needed. |
| `telegram-db insert` | `<name> <table>` | `--rows <json-array>` or `--rows-file <path>` | `drive/telegram/insert` | Inserts row objects into a Telegram DB table. |
| `telegram-db query` | `<name> <sql>` | `--parameters <json-array>` or `--parameters-file <path>` | `drive/telegram/query` | Runs SQL against the named Telegram DB. |
| `telegram-db commit` | `<name>` | none | `drive/telegram/commit` | Commits pending DB changes. |
| `telegram-db metadata` | `<name>` | none | `drive/telegram/metadata` | Returns Telegram DB metadata. |
| `telegram-db drop` | `<name>` | none | `drive/telegram/drop` | Deletes the Telegram DB. |
| `upload` | `<local...>` one or more files/folders | `--dest <dir>`, `--recursive`, `--force` | `drive/upload/start`, PUT upload URL, `drive/upload/commit` per file | Uploads local files into Drive; `--force` overwrites. |
| `mkdir` | `<path>` | none | `drive/paths/create` | Creates a Drive folder. |
| `rm` | `<path>` | `--recursive` | `drive/paths/delete` | Deletes a file or folder. |
| `mv` | `<src> <dst>` | none | `drive/paths/rename` | Renames/moves to destination parent/name. |
| `cp` | `<src> <dst>` | `--force` | Download source bytes, upload destination bytes | Client-side copy. |
| `sync` | `<folder>` | `--dest <dir>`, `--once`, `--interval-seconds <n>` | Repeated local tree upload passes | One-way local-to-Drive sync; no conflict detection. |
| `login` | none | `--dev`, `--app-url <url>`, `--token <token>`, `--credentials-file <path>` | Browser loopback login or OAuth client-credentials exchange | Stores only bearer token under `$DISKD_HOME/credentials`. |
| `logout` | none | none | Local file delete | Deletes stored credentials. |
| `whoami` | none | none | Local JWT payload decode | Prints workspace/subject metadata; never prints token. |
| `set-context` | `[project]` name or id | `--list`, `--root`/`--clear` | Project list REST call or local config write | Project context is local path prefixing, not a Drive API field. |
| `get-context` | none | none | Local config read | Prints current project context or workspace root. |
| `version` | none | none | Local binary metadata | Prints CLI version; JSON mode includes name/version. |
| `update` | none | `--force` | GitHub release lookup, checksum verification, binary replacement | Skipped by agents unless explicitly asked. |
| `mcp serve` | none | none | Embedded MCP stdio server | Direct terminal run prints copyable MCP config; MCP clients launch over stdio. |

## Read and query

### `ls`

```sh
diskd ls [path] [--recursive] [--long] [--show-hidden] [--show-system]
```

Drive method: `paths/tools/ls`. Path defaults to the context root.

Human text output is one row per entry:

```text
<DIR>          0 -              reports (Reports)
<FILE>         5 indexed        a.txt (A Document)
```

The indexing column reads `indexingStatus`/`indexing_status`, or `-` when the
backend omits it. The name column keeps the raw Drive `name` or final path
segment so it can be copied into the next command. When
`displayName`/`display_name` or `metadata.displayName`/`metadata.display_name`
differs, the CLI appends it in parentheses. Use `diskd --json ls` to keep the
backend response unchanged for scripts.

### `tree`

```sh
diskd tree [path] [-L depth] [-a] [-d] [-f] [-s] [--show-system]
```

Drive method: recursive `paths/tools/ls`. Path defaults to the context root.

Useful system-`tree` style flags:

| Flag | Purpose |
| --- | --- |
| `-L`, `--depth`, `--deep <n>` | Limit displayed depth below the root path. |
| `-a`, `--all` | Include hidden Drive entries (`show_hidden=true`). |
| `-d`, `--dirs-only` | Show directories only. |
| `-f`, `--full-path` | Show full Drive paths instead of names plus display metadata. |
| `-s`, `--size` | Show byte size beside each entry. |
| `--show-system` | Include system entries. |

Use `diskd --json tree` to keep the backend recursive listing unchanged for
scripts.

### `glob`

```sh
diskd glob "**/*.pdf" [--path <dir>] [--show-hidden] [--show-system]
```

Drive method: `paths/tools/glob`. `pattern` is required and positional.

### `grep`

```sh
diskd --json grep "payment terms" docs contracts --limit 20 --offset 0
```

Drive method: `paths/tools/grep`. Exact/BM25 search. `query` is required;
zero or more `paths` follow and default to the context root. `--limit <n>` and
`--offset <n>` page matched documents.

`--ignore-case` and `--files-with-matches` are accepted by the parser but
**rejected at runtime** because the Drive grep contract has no matching fields.

### `vsearch`

```sh
diskd --json vsearch "renewal clauses" docs/agreement.pdf --limit 5 --offset 0
```

Drive method: `paths/tools/vsearch`. `query` required; `paths` default to the
context root. `--limit <n>` (alias `--top <n>`) and `--offset <n>` page matched
documents.

Prefer a specific **file** path: directory paths can fail when the backend has
not expanded directory inodes for vector search. Fall back to `grep` for
directory-wide search.

### `cat`

```sh
diskd cat docs/report.pdf > report.pdf
```

Drive method: `drive/files/download-url`, then streams the returned URL to
stdout. `--version <n>` selects a specific file version. Bytes go to stdout, so
redirection and pipes work.

### `read`

```sh
diskd --json read docs/report.pdf --limit 5 --offset 0
```

Drive method: `paths/tools/read`. Returns structured indexed document parts.
`--limit <n>` and `--offset <n>` are aliases for `--parts-limit <n>` and
`--parts-offset <n>` when paging through parts.

### `stat`

```sh
diskd --json stat docs/report.pdf
```

Drive method: `paths/tools/inode-ls` (the deployed path-based metadata surface).

### `biquery`

```sh
diskd --json biquery "total amount grouped by name" docs/table.csv
diskd --json biquery "how many rows have amount over 100?" docs/table.csv
```

Drive method: `paths/tools/bi-query`. The `query` is a **natural-language
question, not SQL**. The Drive backend maps the spreadsheet to a SQLite
database, reads its schema, and uses an LLM (`query_nl_to_sql`) to generate and
execute the SQL, returning a result table. You never write SQL and do not need
to know the table or column names.

Point `paths` at indexed spreadsheet files (`.csv`, `.tsv`, `.xls`, `.xlsx`,
`.mailbox`). Directory paths are expanded to the spreadsheet files inside them;
non-spreadsheet files are ignored, and an all-non-spreadsheet path set returns a
`NO_EXCEL_FILES` error.

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

This is the generic Drive DB working API. It calls `drive/db/*`. Use optional
`--db-type <database|mailbox|telegram|webarchive|session>` when a typed DB needs to be
disambiguated. JSON argument rules:

- `create --schema`/`--schema-file` must be a JSON object.
- `insert --rows`/`--rows-file` must be a JSON array of row objects.
- `query --parameters`/`--parameters-file` must be a JSON array.

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

This is the Telegram SQLite DB working API. It calls the dedicated Drive
Telegram JSON-RPC namespace:

```text
create   -> drive/telegram/create
insert   -> drive/telegram/insert
query    -> drive/telegram/query
commit   -> drive/telegram/commit
metadata -> drive/telegram/metadata
drop     -> drive/telegram/drop
```

JSON argument rules:

- `create --schema`/`--schema-file` must be a JSON object with an `items` array of SQL statements.
- `insert --rows`/`--rows-file` must be a JSON array of row objects.
- `query --parameters`/`--parameters-file` must be a JSON array.

## Write and manage

### `upload`

```sh
diskd upload ./file.txt --dest docs --force
diskd upload ./folder --dest docs --recursive --force
diskd upload ./a.txt ./b.csv --dest docs --force      # multiple local sources
```

Per file: computes SHA-256, calls `drive/upload/start`, PUTs bytes to the
returned upload URL, then calls `drive/upload/commit`. `--dest <dir>` sets the
Drive destination; `--recursive` walks folders; `--force` overwrites existing
targets. Paths are preserved relative to each provided local directory.

### `mkdir`

```sh
diskd mkdir docs
```

Drive method: `drive/paths/create`.

### `rm`

```sh
diskd rm docs/file.txt
diskd rm docs --recursive
```

Drive method: `drive/paths/delete`. `--recursive` deletes folders.

### `mv`

```sh
diskd mv docs/a.txt docs/b.txt
```

Drive method: `drive/paths/rename`.

### `cp`

```sh
diskd cp docs/a.txt docs/copy.txt --force
```

Downloads the source and uploads it to the destination. `--force` overwrites.

### `sync`

```sh
diskd sync ./local-folder --dest docs --once
diskd sync ./local-folder --dest docs --interval-seconds 5
```

One-way local -> Drive. `--once` uploads the current tree and exits. Without
`--once`, it repeats the upload pass on a polling interval (`--interval-seconds`,
default `2`). No conflict detection and no bidirectional sync.

## Auth and meta

### `login`

```sh
diskd login                                          # browser login via app.iosya.com
diskd login --dev                                    # browser login via app.upgraide.dev
diskd login --token "$APIS_ACCESS_TOKEN"                 # store a bearer token
diskd login --credentials-file ./credentials.json        # OAuth client credentials
```

Without flags, the CLI opens the OAuth Apps page and waits for browser-created
diskd CLI credentials on a local callback. With `--credentials-file`, the CLI
reads OIDC discovery from `issuer`, calls the token endpoint with
`grant_type=client_credentials`, and stores the returned access token. It first
requests the gateway scopes used by Drive/project commands; if the issuer rejects
those, it retries with the client's default
scopes. Credential-file input schema and details:
[references/auth-and-config.md](auth-and-config.md).

### `logout`

```sh
diskd logout
```

Deletes the stored credential file.

### `whoami`

```sh
diskd --json whoami
```

Decodes workspace and subject metadata from the current bearer token (display
only, no signature verification). Does not print the token.

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
platform archive and its `.sha256`, verifies the checksum, and replaces the
running binary. `--force` reinstalls the latest even when versions match.

## Project context

### `set-context`

```sh
diskd --json set-context --list        # list projects (GET /v1/platform/projects/api/projects)
diskd set-context "Project Name"       # select by name or id
diskd set-context 01PROJECTID
diskd set-context --root                # clear context, use workspace root (alias: --clear)
```

Stores the selection in `config.yaml` (`project`, `project_name`).

### `get-context`

```sh
diskd --json get-context
```

Prints the stored project context or the workspace-root default.

## MCP

### `mcp serve`

```sh
diskd mcp serve
```

Runs the embedded MCP stdio server. Run directly in a terminal, it prints a
ready-to-copy agent config and exits; launched by an MCP client over stdio, it
speaks the protocol with no human text on stdout. Tools, client config, and
JSON-RPC smoke tests: [references/mcp.md](mcp.md).

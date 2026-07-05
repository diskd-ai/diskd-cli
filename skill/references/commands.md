# diskd Command Reference

Complete flag-by-flag reference. Global flags always precede the subcommand.

## Contents

- [Global flags](#global-flags)
- [Read and query](#read-and-query): `ls`, `glob`, `grep`, `vsearch`, `cat`, `read`, `stat`, `biquery`
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

## Read and query

### `ls`

```sh
diskd ls [path] [--recursive] [--long] [--show-hidden] [--show-system]
```

Drive method: `paths/tools/ls`. Path defaults to the context root.

### `glob`

```sh
diskd glob "**/*.pdf" [--path <dir>] [--show-hidden] [--show-system]
```

Drive method: `paths/tools/glob`. `pattern` is required and positional.

### `grep`

```sh
diskd --json grep "payment terms" docs contracts
```

Drive method: `paths/tools/grep`. Exact/BM25 search. `query` is required;
zero or more `paths` follow and default to the context root.

`--ignore-case` and `--files-with-matches` are accepted by the parser but
**rejected at runtime** because the Drive grep contract has no matching fields.

### `vsearch`

```sh
diskd --json vsearch "renewal clauses" docs/agreement.pdf --top 5
```

Drive method: `paths/tools/vsearch` (sends `top_k`). `query` required; `paths`
default to the context root. `--top <n>` (alias `--limit`) caps results.

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
diskd --json read docs/report.pdf --parts-limit 5 --parts-offset 0
```

Drive method: `paths/tools/read`. Returns structured indexed document parts.
`--parts-limit <n>` and `--parts-offset <n>` page through parts.

### `stat`

```sh
diskd --json stat docs/report.pdf
```

Drive method: `paths/tools/inode-ls` (the deployed path-based metadata surface).

### `biquery`

```sh
diskd --json biquery 'SELECT * FROM "table"' docs/table.csv
diskd --json biquery "total amount grouped by name" docs/table.csv
```

Drive method: `paths/tools/bi-query`. The CLI sends the `query` string and the
spreadsheet paths to Drive. The query can be a plain-language question or
SELECT-style query text; Drive returns a result table.

Point `paths` at indexed spreadsheet files (`.csv`, `.tsv`, `.xls`, `.xlsx`,
`.mailbox`). Directory paths are expanded to the spreadsheet files inside them;
non-spreadsheet files are ignored, and an all-non-spreadsheet path set returns a
`NO_EXCEL_FILES` error.

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
diskd login --token "$APIS_ACCESS_TOKEN"                 # store a bearer token
diskd login --credentials-file ./credentials.json        # OAuth client credentials
```

With `--credentials-file`, the CLI reads OIDC discovery from `issuer`, calls the
token endpoint with `grant_type=client_credentials`, and stores the returned
access token. It first requests the gateway scopes used by Drive/project
commands; if the issuer rejects those, it retries with the client's default
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

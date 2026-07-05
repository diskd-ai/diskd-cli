# Quick Start

This guide gets a new client from installation to reading and searching Drive
files.

## 1. Install

Latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh
```

Pinned release:

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_VERSION=v0.1.5 sh
```

Confirm the binary:

```sh
diskd version
```

Use JSON output by placing `--json` before the subcommand:

```sh
diskd --json version
```

Update later:

```sh
diskd update
```

## 2. Configure API Access

Browser login:

```sh
diskd login
```

This opens `https://app.iosya.com/oauth-apps`. Use `diskd login --dev` for
`https://app.upgraide.dev/oauth-apps`.

For an already-issued bearer token:

```sh
export APIS_BASE_URL="https://apis.iosya.com"
diskd login --token "$APIS_ACCESS_TOKEN"
```

For a CI or service-client credential file:

```sh
diskd login --credentials-file ./credentials.json
```

The credential file format is:

```json
{
  "issuer": "https://auth.example",
  "clientId": "client-id",
  "clientSecret": "client-secret",
  "audience": "diskd-api",
  "apisUrl": "https://apis.iosya.com"
}
```

`diskd` stores only the returned bearer token in `$DISKD_HOME/credentials`.

## 3. Verify Identity

```sh
diskd --json whoami
```

The output includes workspace and subject metadata decoded from the token. It
does not print the token.

## 4. Pick a Project

List accessible projects:

```sh
diskd --json set-context --list
```

Set the current context by project name or id:

```sh
diskd set-context "Project Name"
```

Check the current context:

```sh
diskd get-context
```

Clear the project context and return to the workspace root:

```sh
diskd set-context --root
```

## 5. Upload and Read a File

```sh
printf 'hello diskd\n' > note.txt
diskd mkdir notes
diskd upload ./note.txt --dest notes --force
diskd ls notes
diskd cat notes/note.txt
```

## 6. Search

Exact or BM25 search:

```sh
diskd --json grep "hello diskd" notes
```

Semantic search:

```sh
diskd --json vsearch "greeting note" notes/note.txt --top 5
```

For semantic search, a specific file path is more reliable than a directory
path when the Drive backend has not expanded directory inodes for vector search.

## 7. Query CSV or Spreadsheet Data

```sh
printf 'name,amount\nalpha,10\nbeta,20\n' > table.csv
diskd upload ./table.csv --dest notes --force
diskd --json biquery "what is the total amount?" notes/table.csv
diskd --json biquery "total amount grouped by name" notes/table.csv
```

`biquery` takes a natural-language question. Drive reads the spreadsheet schema,
generates SQL internally, and returns the result table.

## 8. Query a Drive DB

Generic Drive DB:

```sh
diskd --json database query generic-db "SELECT id, text FROM messages LIMIT 20"
```

Typed Telegram Drive DB through the generic API:

```sh
diskd --json database query team-chat "SELECT id, text FROM messages LIMIT 20" --db-type telegram
```

Telegram-specific shortcut:

```sh
diskd --json telegram-db query team-chat "SELECT id, text FROM messages LIMIT 20"
```

Use `database create`, `insert`, `commit`, `rollback`, `metadata`, `drop`, and
resolver commands for generic Drive DBs. Use `telegram-db create`, `insert`,
`commit`, `metadata`, and `drop` for Telegram SQLite DB shortcuts.

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
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | DISKD_VERSION=v0.1.3 sh
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
diskd --json biquery 'SELECT SUM(amount) AS total FROM "table"' notes/table.csv
```

The table name is produced by Drive indexing. If unsure, inspect SQLite tables:

```sh
diskd --json biquery "SELECT name FROM sqlite_master WHERE type='table'" notes/table.csv
```

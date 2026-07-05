# How-Tos

## Use a Different Gateway

For one command:

```sh
diskd --base-url https://apis.example --json set-context --list
```

For the shell session:

```sh
export APIS_BASE_URL="https://apis.example"
diskd --json set-context --list
```

For future commands, write it through login with a credential file whose
`apisUrl` points at the gateway.

## Keep State in a Project Directory

Use `DISKD_HOME` to isolate credentials and config:

```sh
export DISKD_HOME="$PWD/.diskd"
diskd login --token "$APIS_ACCESS_TOKEN"
diskd set-context "Project Name"
```

This is useful for CI and test runs.

## Upload a Folder Once

```sh
diskd upload ./docs --dest imported-docs --recursive --force
```

`upload` preserves paths relative to each provided local directory.

## Continuously Push a Local Folder

```sh
diskd sync ./docs --dest imported-docs --interval-seconds 10
```

This is a simple polling loop. It does not implement conflict detection or
bidirectional sync.

## Download a File

```sh
diskd cat docs/report.pdf > report.pdf
```

`cat` writes file bytes to stdout, so redirection and pipes work as expected.

## Search Indexed Content

Exact or BM25 search:

```sh
diskd --json grep "invoice total" docs
```

Semantic search:

```sh
diskd --json vsearch "documents about payment deadlines" docs/invoice.pdf --top 5
```

If a directory-level vector search fails because the backend reports a directory
without a file id, retry with a specific file path or use `grep` for directory
search.

## Ask Questions of a CSV or Spreadsheet

Upload a CSV:

```sh
printf 'name,amount\nalpha,10\nbeta,20\n' > table.csv
diskd upload ./table.csv --dest data --force
```

Ask natural-language questions:

```sh
diskd --json biquery "what is the total amount?" data/table.csv
diskd --json biquery "which name has the highest amount?" data/table.csv
```

## Work with a Telegram Drive DB

Create a Telegram DB:

```sh
diskd --json telegram-db create team-chat \
  --schema '{"items":["CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"]}'
```

Insert rows:

```sh
diskd --json telegram-db insert team-chat messages \
  --rows '[{"id":1,"text":"hello"}]'
```

Run a SQL query against the named Telegram DB:

```sh
diskd --json telegram-db query team-chat \
  "SELECT id, text FROM messages WHERE text = ?" \
  --parameters '["hello"]'
```

Commit pending changes and inspect metadata:

```sh
diskd --json telegram-db commit team-chat
diskd --json telegram-db metadata team-chat
```

## Use JSON Output in Scripts

Put `--json` before the command:

```sh
diskd --json ls docs
```

Pipe into `jq`:

```sh
diskd --json set-context --list | jq -r '.[].name'
```

## Update the CLI

```sh
diskd update
```

When a newer release exists, normal human-facing commands print a yellow stderr
notice. Disable startup checks for a shell session:

```sh
export DISKD_NO_UPDATE_CHECK=1
```

## Add diskd to an LLM Agent

Run:

```sh
diskd mcp serve
```

If the command is run directly in a terminal, it prints the MCP server
configuration to add to an LLM agent. After the agent is configured, the agent
launches `diskd mcp serve` over stdio.

## Run in CI

```sh
export DISKD_HOME="$RUNNER_TEMP/diskd"
export APIS_BASE_URL="https://apis.iosya.com"
diskd login --token "$APIS_ACCESS_TOKEN"
diskd --json whoami
diskd --json upload ./artifact.txt --dest ci --force
```

Prefer short-lived tokens in CI and remove `$DISKD_HOME` after the job.

## Troubleshoot Auth

Check that the CLI can decode workspace metadata:

```sh
diskd --json whoami
```

If gateway calls return 401 or 403:

```sh
diskd logout
diskd login --token "$APIS_ACCESS_TOKEN"
diskd --json set-context --list
```

The token must be accepted by `apis-service` and authorized for the Drive or
project route you call.

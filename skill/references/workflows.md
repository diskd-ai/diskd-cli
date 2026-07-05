# diskd Workflows and Recipes

End-to-end patterns for common tasks. All commands assume `diskd` is on `PATH`
and a token is available (see [auth-and-config.md](auth-and-config.md)).

## Contents

- [Upload, list, and read a file](#upload-list-and-read-a-file)
- [Search indexed content](#search-indexed-content)
- [Query a CSV or spreadsheet in plain English (biquery)](#query-a-csv-or-spreadsheet-in-plain-english-biquery)
- [Upload a folder once](#upload-a-folder-once)
- [Continuously push a local folder](#continuously-push-a-local-folder)
- [Use a different gateway](#use-a-different-gateway)
- [Isolate state in a project directory](#isolate-state-in-a-project-directory)
- [JSON output in scripts](#json-output-in-scripts)
- [Run in CI](#run-in-ci)
- [Add diskd to an LLM agent](#add-diskd-to-an-llm-agent)
- [Troubleshoot auth](#troubleshoot-auth)

## Upload, list, and read a file

```sh
printf 'hello diskd\n' > note.txt
diskd mkdir notes
diskd upload ./note.txt --dest notes --force
diskd ls notes
diskd cat notes/note.txt
diskd --json read notes/note.txt --parts-limit 3
```

## Search indexed content

Exact or BM25 search over a directory:

```sh
diskd --json grep "invoice total" docs
```

Semantic search -- prefer a specific file path:

```sh
diskd --json vsearch "documents about payment deadlines" docs/invoice.pdf --top 5
```

If a directory-level vector search fails because the backend reports a directory
without a file id, retry with a specific file path or use `grep` for the
directory.

## Query a CSV or spreadsheet in plain English (biquery)

`biquery` takes a **natural-language question, not SQL**. The Drive backend maps
the spreadsheet to SQLite, reads the schema, and uses an LLM to write and run the
SQL for you. You never write SQL or reference table/column names directly.

Upload the data:

```sh
printf 'name,amount\nalpha,10\nbeta,20\n' > table.csv
diskd upload ./table.csv --dest data --force
```

Ask questions in natural language:

```sh
diskd --json biquery "what is the total amount?" data/table.csv
diskd --json biquery "total amount grouped by name" data/table.csv
diskd --json biquery "which name has the highest amount?" data/table.csv
```

Supported inputs are indexed spreadsheet files (`.csv`, `.tsv`, `.xls`, `.xlsx`,
`.mailbox`). A directory path is expanded to the spreadsheet files inside it.

## Upload a folder once

```sh
diskd upload ./docs --dest imported-docs --recursive --force
```

`upload` preserves paths relative to each provided local directory.

## Continuously push a local folder

```sh
diskd sync ./docs --dest imported-docs --interval-seconds 10
```

A simple polling loop, one-way local -> Drive. No conflict detection, no
bidirectional sync. Use `--once` to upload the current tree and exit.

## Use a different gateway

One command:

```sh
diskd --base-url https://apis.example --json set-context --list
```

For the shell session:

```sh
export APIS_BASE_URL="https://apis.example"
diskd --json set-context --list
```

To persist it, log in with a credential file whose `apisUrl` points at the
gateway.

## Isolate state in a project directory

```sh
export DISKD_HOME="$PWD/.diskd"
diskd login --token "$APIS_ACCESS_TOKEN"
diskd set-context "Project Name"
```

Useful for CI and test runs; remove `$DISKD_HOME` afterward.

## JSON output in scripts

Put `--json` before the command and pipe into `jq`:

```sh
diskd --json ls docs
diskd --json set-context --list | jq -r '.[].name'
```

## Run in CI

```sh
export DISKD_HOME="$RUNNER_TEMP/diskd"
export APIS_BASE_URL="https://apis.iosya.com"
diskd login --token "$APIS_ACCESS_TOKEN"
diskd --json whoami
diskd --json upload ./artifact.txt --dest ci --force
```

Prefer short-lived tokens and remove `$DISKD_HOME` after the job.

## Add diskd to an LLM agent

```sh
diskd mcp serve
```

Run directly in a terminal, this prints the MCP server config to add to an
agent. Once configured, the agent launches `diskd mcp serve` over stdio. Full
config examples: [mcp.md](mcp.md).

## Troubleshoot auth

Confirm the CLI can decode workspace metadata:

```sh
diskd --json whoami
```

On 401/403 from gateway calls, re-authenticate:

```sh
diskd logout
diskd login --token "$APIS_ACCESS_TOKEN"
diskd --json set-context --list
```

The token must be accepted by `apis-service` and authorized for the Drive or
project route you call.

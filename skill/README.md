# diskd CLI Skill

> **Install:** `npx skills add diskd-ai/diskd-cli` | [skills.sh](https://skills.sh)

Skill for driving `diskd`, the Rust command-line client for the diskd Drive API,
through the public `apis-service` gateway. Gives agents and scripts a Unix-style
interface for Drive files: list, read, search, upload, sync, and query.

---

## Scope & Purpose

This skill provides commands, flags, and end-to-end patterns for `diskd`:

* Browse Drive paths (`ls`, `tree`, `glob`, `stat`) and stream bytes (`cat`)
* Read structured indexed document parts (`read`)
* Exact/BM25 search (`grep`) and semantic search (`vsearch`)
* Natural-language (plain-English) questions over indexed CSV/TSV/XLS/XLSX/mailbox spreadsheets (`biquery`); the Drive backend turns the question into SQL and runs it
* Upload, folder create, rename, copy, remove, and one-way sync
* Auth (`login`/`logout`/`whoami`), project context (`set-context`/`get-context`)
* Self-update (`update`) and the embedded MCP stdio server (`mcp serve`)
* Human text output by default, `--json` for scripts

---

## When to Use This Skill

**Triggers:**

* Mentions of diskd, the diskd CLI, or the diskd drive
* Commands like `diskd ls / tree / cat / read / grep / vsearch / biquery / upload / sync`
* Uploading files to, or searching/reading files on, the diskd Drive from a shell
* Asking natural-language questions of CSV or spreadsheet files stored in Drive
* Adding diskd as an MCP server to an LLM agent

**Use cases:**

* Automating Drive file operations in scripts or CI with `--json`
* Semantic (`vsearch`) and exact (`grep`) search over indexed content
* Querying tabular data in plain English with `biquery` (no SQL to write)
* Wiring diskd tools into an agent via `diskd mcp serve`

---

## Quick Reference

### Install the binary

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh
diskd version
```

### Authenticate and pick a project

```sh
export APIS_BASE_URL="https://apis.iosya.com"
diskd login --token "$APIS_ACCESS_TOKEN"
diskd --json whoami
diskd --json set-context --list
diskd set-context "Project Name"
```

### Work with files

```sh
diskd mkdir docs
diskd upload ./report.pdf --dest docs --force
diskd ls docs
diskd cat docs/report.pdf > report.pdf
diskd --json grep "payment terms" docs
diskd --json vsearch "renewal clauses" docs/report.pdf --top 5
diskd --json biquery "what is the total amount?" docs/table.csv
```

Global flags (e.g. `--json`, `-p`, `--base-url`) must come **before** the
subcommand: `diskd --json ls docs`.

---

## Skill Structure

```
skill/
  SKILL.md                      # Command shape, quick start, command surface, key behaviors
  README.md                     # This overview
  references/
    commands.md                 # Complete flag-by-flag reference + Drive method per command
    auth-and-config.md          # login, credentials, config.yaml, env vars, path context
    mcp.md                      # MCP stdio server: tools, client config, JSON-RPC smoke tests
    workflows.md                # End-to-end recipes: search, biquery, CI, sync, agent, troubleshoot
```

---

## Resources

* **Full skill reference**: [SKILL.md](SKILL.md)
* **Command reference**: [references/commands.md](references/commands.md)
* **Auth & configuration**: [references/auth-and-config.md](references/auth-and-config.md)
* **MCP server**: [references/mcp.md](references/mcp.md)
* **Workflows**: [references/workflows.md](references/workflows.md)
* **Repository**: https://github.com/diskd-ai/diskd-cli

# Development and Releases

The repository is a Rust workspace with three crates:

| Crate | Purpose |
| --- | --- |
| `diskd-cli` | Binary, command parsing, rendering, local file I/O, upload/sync, MCP stdio. |
| `diskd-client` | Typed Drive and project gateway client plus JSON-RPC request builders. |
| `diskd-config` | Config parsing, path normalization, credential document parsing, JWT display decoding. |

## Local Checks

```sh
cargo fmt --all
cargo test --workspace
cargo clippy --workspace -- -D warnings
sh -n install.sh
```

Tests include:

- request-builder unit tests for Drive JSON-RPC wire contracts;
- config/path/JWT unit tests;
- fake-gateway binary E2E tests that run the compiled `diskd` executable.

## Manual Dev-Stand Smoke

Use an isolated state directory:

```sh
export DISKD_HOME="$(mktemp -d)"
diskd login --credentials-file ./credentials.json
diskd --json whoami
diskd --json set-context --list
diskd set-context "Project Name"
```

Create an isolated test folder:

```sh
stamp="diskd-cli-e2e-$(date +%Y%m%d%H%M%S)"
diskd mkdir "$stamp"
```

Exercise the main flow:

```sh
printf 'diskd smoke test\n' > note.txt
printf 'name,amount\nalpha,10\nbeta,20\n' > table.csv
diskd upload ./note.txt ./table.csv --dest "$stamp" --force
diskd --json ls "$stamp"
diskd cat "$stamp/note.txt"
diskd --json grep "diskd smoke" "$stamp"
diskd --json vsearch "smoke test" "$stamp/note.txt" --top 3
diskd --json biquery 'SELECT SUM(amount) AS total FROM "table"' "$stamp/table.csv"
diskd rm "$stamp" --recursive
```

## Release Workflow

`.github/workflows/release.yml` builds these targets:

```text
x86_64-unknown-linux-musl
aarch64-unknown-linux-musl
x86_64-apple-darwin
aarch64-apple-darwin
x86_64-pc-windows-msvc
```

Each target produces:

```text
diskd-<version>-<target>.tar.gz
diskd-<version>-<target>.tar.gz.sha256
```

Push a tag to release:

```sh
git tag v0.1.2
git push github v0.1.2
```

Run manually:

```sh
gh workflow run release --repo diskd-ai/diskd-cli --field version=v0.1.2 --ref main
```

Verify:

```sh
gh release view v0.1.2 --repo diskd-ai/diskd-cli
```

The self-update command depends on these exact release asset names and matching
`.sha256` files. Keep the workflow archive contract stable when changing the
release process.

## Installer Smoke

```sh
tmp="$(mktemp -d)"
DISKD_INSTALL_DIR="$tmp" DISKD_VERSION=v0.1.2 sh install.sh
"$tmp/diskd" --json version
```

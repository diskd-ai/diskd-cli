# diskd CLI

`diskd` is the agent-native command-line client for the diskd drive.

This repository is intended to be published as the public
`diskd-ai/diskd-cli` GitHub repository. Release tags build platform binaries
and publish checksummed archives to GitHub Releases.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh
```

Pin a release:

```sh
DISKD_VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/diskd-ai/diskd-cli/main/install.sh | sh
```

## Development

```sh
cargo test --workspace
cargo clippy --workspace -- -D warnings
```


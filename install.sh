#!/usr/bin/env sh
set -eu

REPO="${DISKD_REPO:-diskd-ai/diskd-cli}"
INSTALL_DIR="${DISKD_INSTALL_DIR:-}"
VERSION="${DISKD_VERSION:-}"

fail() {
  printf '%s\n' "diskd install: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      os_part="unknown-linux-musl"
      ;;
    Darwin)
      os_part="apple-darwin"
      ;;
    *)
      fail "unsupported OS: $os"
      ;;
  esac

  case "$arch" in
    x86_64 | amd64)
      arch_part="x86_64"
      ;;
    arm64 | aarch64)
      arch_part="aarch64"
      ;;
    *)
      fail "unsupported architecture: $arch"
      ;;
  esac

  printf '%s-%s\n' "$arch_part" "$os_part"
}

resolve_version() {
  if [ -n "$VERSION" ]; then
    printf '%s\n' "$VERSION"
    return
  fi

  need curl
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -n 1
}

resolve_install_dir() {
  if [ -n "$INSTALL_DIR" ]; then
    printf '%s\n' "$INSTALL_DIR"
    return
  fi

  if [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
    printf '%s\n' "/usr/local/bin"
    return
  fi

  printf '%s\n' "$HOME/.local/bin"
}

verify_checksum() {
  asset="$1"
  checksum="$2"

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$checksum"
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$checksum"
    return
  fi

  fail "missing checksum command: shasum or sha256sum"
}

main() {
  need curl
  need tar

  target="$(detect_target)"
  version="$(resolve_version)"
  [ -n "$version" ] || fail "could not resolve release version"

  asset="diskd-${version}-${target}.tar.gz"
  base_url="https://github.com/$REPO/releases/download/$version"
  tmp_dir="$(mktemp -d)"
  install_dir="$(resolve_install_dir)"

  trap 'rm -rf "$tmp_dir"' EXIT INT TERM

  printf '%s\n' "Installing diskd $version for $target"
  curl -fsSL "$base_url/$asset" -o "$tmp_dir/$asset"
  curl -fsSL "$base_url/$asset.sha256" -o "$tmp_dir/$asset.sha256"

  (cd "$tmp_dir" && verify_checksum "$asset" "$asset.sha256")
  tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"

  mkdir -p "$install_dir"
  cp "$tmp_dir/diskd" "$install_dir/diskd"
  chmod 0755 "$install_dir/diskd"

  printf '%s\n' "diskd installed to $install_dir/diskd"
}

main "$@"


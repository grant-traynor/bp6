#!/usr/bin/env zsh
# Download restate-server to poe/bin/ for local dev.
# Runs automatically from build_run.sh; re-run any time to upgrade.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$SCRIPT_DIR/../bin"
DEST="$BIN_DIR/restate-server"

# Already present?
if [[ -x "$DEST" ]]; then
  echo "✅ restate-server already at $DEST"
  exit 0
fi

# Determine target triple
ARCH="$(uname -m)"   # arm64 or x86_64
OS="$(uname -s)"     # Darwin or Linux

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  TRIPLE="aarch64-apple-darwin" ;;
      x86_64) TRIPLE="x86_64-apple-darwin" ;;
      *) echo "❌ Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      aarch64) TRIPLE="aarch64-unknown-linux-musl" ;;
      x86_64)  TRIPLE="x86_64-unknown-linux-musl" ;;
      *) echo "❌ Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "❌ Unsupported OS: $OS"
    exit 1
    ;;
esac

VERSION="$(curl -fsSL https://api.github.com/repos/restatedev/restate/releases/latest \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])")"

URL="https://github.com/restatedev/restate/releases/download/${VERSION}/restate-server-${TRIPLE}.tar.xz"

echo "⬇️  Downloading restate-server ${VERSION} for ${TRIPLE}…"
mkdir -p "$BIN_DIR"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" -o "$TMP/restate.tar.xz"
tar -xJf "$TMP/restate.tar.xz" -C "$TMP"

# The archive contains a single file or a dir — find the binary
BINARY="$(find "$TMP" -name "restate-server" -type f | head -1)"
if [[ -z "$BINARY" ]]; then
  echo "❌ Could not find restate-server binary in archive"
  ls "$TMP"
  exit 1
fi

cp "$BINARY" "$DEST"
chmod +x "$DEST"

echo "✅ restate-server ${VERSION} installed at $DEST"

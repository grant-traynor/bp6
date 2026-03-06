#!/usr/bin/env zsh
set -e

cd "$(dirname "$0")"

# Download restate-server if not already present
scripts/setup-restate.sh

npm run tauri build -- --debug
open src-tauri/target/debug/bundle/macos/poe.app

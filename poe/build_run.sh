#!/usr/bin/env zsh
set -e

cd "$(dirname "$0")"

npm run tauri build -- --debug
open src-tauri/target/debug/bundle/macos/poe.app

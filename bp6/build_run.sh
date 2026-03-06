#!/usr/bin/env zsh

npm run tauri build -- --debug
open src-tauri/target/debug/bundle/macos/bp6.app

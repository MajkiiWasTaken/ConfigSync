#!/usr/bin/env bash
# /************************************************
# * File: install.sh
# * Author: Michal Švrček
# * Linux per-user installer
# * ver. 0.4.0
# *************************************************/
set -euo pipefail
source_binary="${1:-$(dirname "$(dirname "$(realpath "$0")")")/target/release/csync}"
[[ -f "$source_binary" ]] || { echo "Binary missing: $source_binary" >&2; exit 1; }
mkdir -p "$HOME/.local/bin"
install -m 755 "$source_binary" "$HOME/.local/bin/csync"
echo '[OK] Installed ~/.local/bin/csync (ensure this directory is in PATH)'

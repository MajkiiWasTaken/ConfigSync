#!/usr/bin/env bash
# /************************************************
# * File: uninstall.sh
# * Author: Michal Švrček
# * Linux per-user uninstaller (preserves settings/backups)
# * ver. 0.4.0
# *************************************************/
set -euo pipefail
rm -f "$HOME/.local/bin/csync"
echo '[OK] Removed csync binary; configuration and backups retained.'

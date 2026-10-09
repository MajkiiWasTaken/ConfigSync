# ConfigSync

**ConfigSync v0.7.0** is a Rust command-line tool for backing up, comparing, exporting, importing, and migrating selected development configurations between Windows and Linux machines. It uses explicit TOML profiles and colorful terminal output.

> **Security notice:** Backup folders and exported ZIP archives are **not encrypted**. Never include passwords, tokens, private SSH keys, credential stores, or browser profiles. Only import archives from trusted sources; SHA-256 verifies content against the manifest but does not authenticate its author.

## Features

- Opt-in backup of individual files and entire directories
- SHA-256 checksums for modern backup snapshots
- Read-only discovery of common development configuration locations
- Multiple profiles and TOML configuration management
- Preview-first restore, with recovery copies of overwritten files
- ZIP export/import, with archive path validation and resource limits
- Snapshot comparison, file counts, and backup sizes
- Cross-machine migration using the **receiving machine's profile paths**
- Selective migration with `--only` and overwrite avoidance with `--skip-existing` (v0.7.0)
- Windows and Linux install/uninstall scripts, and GitHub Actions release workflow

## Installation

### Prerequisites

- Rust and Cargo (when building from source)
- Windows 10/11 or a recent Linux distribution

```powershell
cargo fmt --check
cargo test
cargo build --release
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

On Linux:

```bash
cargo fmt --check
cargo test
cargo build --release
bash scripts/install.sh
```

Open a fresh terminal and run `csync --version`.

### Initial configuration

```powershell
csync init --scan
csync profiles
csync paths
```

`init --scan` works only if a config file does not already exist. **Review every detected path before backing up.** Alternatively run `csync init` to create a new config and manually add selected entries.

Config location:

- Windows: `%APPDATA%\ConfigSync\config.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/configsync/config.toml`

Backups:

- Windows: `%LOCALAPPDATA%\ConfigSync\backups`
- Linux: `${XDG_DATA_HOME:-~/.local/share}/configsync/backups`

You can override the configuration file using `--config PATH`.

## Configuration

```toml
[profiles.development]
description = "Development configuration"

[profiles.development.files]
gitconfig = "~/.gitconfig"
vscode_snippets = '${APPDATA}/Code/User/snippets'
```

A profile entry can refer to an individual file or a directory. Directories are copied recursively (except symlinks, which are rejected). Variable substitution supports `${ENVIRONMENT_VARIABLE}` and the home-directory prefix `~`.

On Linux, use paths appropriate to the receiving system instead of `${APPDATA}`.

## CLI commands

| Command | Description |
| --- | --- |
| `csync init` | Initialize configuration without overwriting it |
| `csync init --scan` | Initialize a profile using discovered locations |
| `csync scan` | Show available configuration locations without changing them |
| `csync profiles` | List profiles |
| `csync profile add work --description "Work settings"` | Create a profile |
| `csync profile set work gitconfig '~/.gitconfig'` | Add a source |
| `csync profile unset work gitconfig` | Remove a source |
| `csync profile remove work --yes` | Remove a profile (not its backup history) |
| `csync info development` | Show profile paths |
| `csync profile-show development` | Show file counts and sizes |
| `csync backup development --dry-run` | Preview backup |
| `csync backup development` | Create snapshot |
| `csync list development` | List snapshots |
| `csync diff development` | Compare source files to latest snapshot |
| `csync restore development --dry-run` | Preview restore |
| `csync restore development --yes` | Restore with recovery copies |
| `csync export development --output development.zip` | Create portable ZIP |
| `csync import development.zip` | Validate/import ZIP into local backups |
| `csync migrate development` | Preview migration |
| `csync migrate development --yes` | Apply migration |
| `csync paths` | Display config/backup directories |

Most snapshot commands support `--id BACKUP_ID` to select a specific snapshot.

## Migrating to another computer

On the source computer:

```powershell
csync backup development
csync export development --output development.zip
```

On the destination computer:

1. Install ConfigSync and create a profile with **the same profile name and aliases**, but destinations appropriate to the new system.
2. Transfer the ZIP file through a trusted channel.
3. Import it and preview before writing.

```powershell
csync import .\development.zip
csync list development
csync migrate development
```

### Select specific configurations (v0.7.0)

```powershell
csync migrate development --only gitconfig
csync migrate development --only gitconfig --only vscode_snippets
```

The repeated `--only` option restricts migration to selected **profile aliases**, not individual files inside an alias.

### Keep existing target files (v0.7.0)

```powershell
csync migrate development --skip-existing
csync migrate development --only vscode_snippets --skip-existing
```

To apply the reviewed plan, append `--yes`:

```powershell
csync migrate development --only gitconfig --skip-existing --yes
```

An empty selection is treated as an error. Only snapshots with modern manifest entries support selective migration. During actual restoration, ConfigSync validates the backup and writes recovery copies of any files it overwrites.

## Safety and limitations

- No command modifies configuration files during `scan`, `diff`, `list`, migration preview, or restore preview.
- `restore --yes` and `migrate --yes` can overwrite real files: carefully inspect the local profile and preview first.
- The snapshot manifest provides SHA-256 integrity checks for modern backups. Historical v0.1.0 backups do not have recorded checksums.
- The importer refuses unsafe ZIP paths, unsupported entries, duplicates, and oversized archives. It is **not** an authenticity or antivirus check.
- Symbolic links are rejected; empty directories are not retained.
- Recovery copies are made before overwriting original files. **This is not a fully transactional restore**: manual intervention may be required if power or I/O fails.
- **There is no encryption, authentication of backups, interactive per-file selection, or reliable automatic translation between arbitrary Windows/Linux application layouts** in v0.7.0.
- For sensitive files or untrusted archives, use a different encrypted and authenticated backup solution.

## Building and testing

```powershell
cargo fmt --check
cargo check
cargo test
cargo build --release
cargo run -- --version
cargo run -- scan
cargo run -- migrate development
```

GitHub Actions packages Windows `.zip` and Linux `.tar.gz` builds on version tags (`v0.7.0`). Do not publish a final release until checks pass on both platforms.

## Author

**Michal Švrček** — [MajkiiWasTaken](https://github.com/MajkiiWasTaken)

## License

MIT License. See [LICENSE](LICENSE).

# ConfigSync v0.4.0

A colored Rust CLI for opt-in configuration backup and restore on Windows and Linux.

## Setup

```powershell
cargo build
New-Item -ItemType Directory -Force "$env:APPDATA\ConfigSync"
Copy-Item config.example.toml "$env:APPDATA\ConfigSync\config.toml"
cargo run -- scan
cargo run -- profiles
```

Edit `config.toml` to select the **files or folders** you actually want to back up. `${APPDATA}` and `~` variables are supported. Avoid credentials, tokens, private SSH keys, and other secrets because backups are **unencrypted**.

## Commands

```text
csync scan
csync profiles
csync info development
csync backup development --dry-run
csync backup development
csync list development
csync restore development --dry-run
csync restore development --id BACKUP_ID --dry-run
csync restore development --yes
csync paths
```

## Safety and behavior

- `scan` only detects well-known paths. It does **not** update the profile or copy anything.
- Backups recursively include regular files in configured directories. Symbolic links are rejected.
- All v0.3.0 backup entries have SHA-256 hashes checked **before any restoration**.
- Existing destination files are preserved under the backups `recovery/` directory before they are replaced.
- Restore previews by default; `--yes` explicitly opts in to overwrite.
- Existing v0.1.0 backups can still be restored by alias, but those historical backups have **no stored hashes**.
- Directory recursion copies all regular files, so avoid selecting sensitive directories such as `.ssh`, password stores, and browser profiles.
- This version does not provide transactional rollback, encryption, automatic ZIP transport, empty-directory preservation, or cross-machine path rewriting. Restoration can be partially applied if I/O fails, but the overwritten originals are saved in a recovery directory.
- `--config PATH` allows a custom config file.

## Author

Michal Švrček

## Portable ZIP transfer (v0.3.0)

Export a backup created with `csync backup development`:

```powershell
csync export development --output development.zip
csync export development --id BACKUP_ID --output older.zip
```

On another computer, install ConfigSync, create/edit `%APPDATA%\ConfigSync\config.toml` with the **new machine's destination paths**, then:

```powershell
csync import development.zip
csync list development
csync restore development --dry-run
csync restore development --yes
```

Import NEVER overwrites target configuration files. Restore only changes them with `--yes`; originals are saved to a recovery directory. Import assigns a new backup ID. Old v0.1 snapshots may be exported, but they lack recorded SHA-256 hashes. ZIP files are **not encrypted**. Do not distribute configuration secrets.

ZIP importer rejects absolute/traversal paths, symlinks, unexpected entries, duplicate names and large entries; validates SHA-256 against manifest before committing a snapshot. Limits: 20,000 files, 256 MiB per file, 1 GiB total. Do not import archives from untrusted sources without independent verification: checksums detect corruption, not authenticity.

Empty folders are not preserved. Profile paths are supplied locally, never written into an archive. A snapshot can only be restored using a configured matching profile with matching aliases.

## ConfigSync v0.4.0 — Profile management and installers

Initialize a configuration once (never overwrites an existing configuration):

```powershell
csync init
csync profile add development --description "Developer settings"
csync profile set development gitconfig '~/.gitconfig'
csync profile set development snippets '${APPDATA}/Code/User/snippets'
csync profiles
csync info development
csync profile unset development snippets
csync profile remove development --yes
```

`profile set` requires an existing local file/directory and refuses to overwrite an existing alias. The config is backed up as `config.toml.bak` before modifications. Removing profiles does **not** delete saved backups. A new empty configuration/profile is permitted, but backup of an empty profile is rejected.

Windows: run `cargo build --release`, then `powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1`. Linux: run `cargo build --release`, then `bash scripts/install.sh`. Open a new terminal to refresh PATH. Installer/uninstaller do not remove the saved backups or configuration.

Note: `scan` is read-only and does not auto-enroll sensitive configuration. ZIP export/import remains unencrypted.

## New in v0.5.0

- `csync init --scan` creates a new configuration with discovered development files (never overwrites an existing configuration). Review the selections before backup; unencrypted backup copies can contain sensitive information.
- `csync profile-show development` lists each configured source with file count and byte count.
- `csync diff development` compares current file contents to the newest v0.2+ backup, verifying backup SHA-256 hashes first. Use `--id BACKUP_ID` to compare with an older backup.
- `csync list development` shows sizes of local backup directories.
- GitHub Actions builds Windows and Linux release archives when a version tag is pushed.

### Build and test

```powershell
cargo fmt
cargo check
cargo test
cargo build --release
cargo run -- profile-show development
cargo run -- diff development
```

`diff` does not restore files. `init --scan` refuses to overwrite an existing `config.toml`.

## v0.6.0: Migration planner

Transfer a profile using an existing ZIP backup, and configure destination paths
in the receiving computer's `config.toml`. The import command **does not restore**.

```powershell
csync import .\development.zip
csync list development
csync migrate development
csync migrate development --yes
```

`migrate` examines the latest backup unless `--id BACKUP_ID` is provided.
It validates backup SHA-256 checksums, maps stored aliases to paths in the *local*
profile, rejects unsafe existing destination links, and labels the destination
files as `NEW` or `OVERWRITE`. It does not write without `--yes`.

When authorized, migration invokes the existing restore engine, which saves
copies of existing files under the ConfigSync backups recovery directory.
This is **not** a fully transactional rollback; interrupted restores may require
manual recovery. Backup archives are not encrypted or authenticated, so do not
import archives from untrusted parties or include secrets such as SSH keys.

For Windows/Linux migrations, create the equivalent profile on the receiving OS
and explicitly set platform-appropriate absolute destination paths. Windows-only
application settings may not be portable to Linux and vice versa.

The `migrate` command does not automate interactive selections or encryption in
v0.6.0; those are planned improvements.

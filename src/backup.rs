/************************************************
* File: backup.rs
* Author: Michal Švrček
*
* Profile backups, integrity checks and safe restore
*
* ver. 0.4.0
*************************************************/

use crate::{
    config::{self, Profile},
    output,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub profile: String,
    pub created: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>, // v0.1.0 compatibility
    #[serde(default)]
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub alias: String,
    pub relative: String,
    pub sha256: String,
}

fn backups_for(name: &str) -> Result<PathBuf, String> {
    config::valid_name(name)?;
    Ok(config::data_dir()?.join(name))
}
fn hash(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut sha = Sha256::new();
    let mut chunk = [0u8; 32768];
    loop {
        let n = f.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        sha.update(&chunk[..n]);
    }
    Ok(format!("{:x}", sha.finalize()))
}
fn clean_relative(p: &Path) -> bool {
    !p.as_os_str().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}
fn scan_source(alias: &str, source: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let meta = fs::symlink_metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("Symbolic link not supported: {}", source.display()));
    }
    if meta.is_file() {
        return Ok(vec![(
            PathBuf::from(alias).join("__file__"),
            source.to_path_buf(),
        )]);
    }
    if !meta.is_dir() {
        return Err(format!("Not a file or directory: {}", source.display()));
    }
    let mut out = Vec::new();
    for item in WalkDir::new(source).follow_links(false).into_iter() {
        let item = item.map_err(|e| e.to_string())?;
        if item.file_type().is_symlink() {
            return Err(format!(
                "Symbolic link not supported: {}",
                item.path().display()
            ));
        }
        if !item.file_type().is_file() {
            continue;
        }
        let rel = item
            .path()
            .strip_prefix(source)
            .map_err(|e| e.to_string())?;
        if !clean_relative(rel) {
            return Err("Unsafe relative path".into());
        }
        out.push((PathBuf::from(alias).join(rel), item.path().to_path_buf()));
    }
    Ok(out)
}

pub fn create(name: &str, profile: &Profile, dry_run: bool) -> Result<(), String> {
    if profile.files.is_empty() {
        return Err(format!(
            "Profile {name} is empty. Use csync profile set first."
        ));
    }
    output::banner();
    output::heading(&format!("Backup: {name}"));
    let mut sources = Vec::new();
    for (alias, raw) in &profile.files {
        config::valid_name(alias)?;
        let source = config::expand_path(raw)?;
        let files = scan_source(alias, &source)?;
        output::step(&format!(
            "{alias}: {} ({} file(s))",
            source.display(),
            files.len()
        ));
        sources.extend(files);
    }
    if dry_run {
        output::warn("DRY RUN: No backup created");
        return Ok(());
    }
    let now = Utc::now();
    let suffix = now.format("%Y%m%dT%H%M%S%.fZ").to_string();
    let base = backups_for(name)?.join(suffix);
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let result = (|| -> Result<Manifest, String> {
        let mut entries = Vec::new();
        for (relative, source) in sources {
            let dest = base.join("data").join(&relative);
            fs::create_dir_all(dest.parent().ok_or("Bad backup path")?)
                .map_err(|e| e.to_string())?;
            fs::copy(&source, &dest).map_err(|e| format!("Copy {}: {e}", source.display()))?;
            entries.push(Entry {
                alias: relative
                    .components()
                    .next()
                    .unwrap()
                    .as_os_str()
                    .to_string_lossy()
                    .into_owned(),
                relative: relative.to_string_lossy().replace('\\', "/"),
                sha256: hash(&dest)?,
            });
        }
        let manifest = Manifest {
            profile: name.into(),
            created: now.to_rfc3339(),
            files: BTreeMap::new(),
            entries,
        };
        fs::write(
            base.join("manifest.toml"),
            toml::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(manifest)
    })();
    if let Err(ref e) = result {
        let _ = fs::remove_dir_all(&base);
        output::error(e);
    }
    let manifest = result?;
    output::success(&format!(
        "Backup created: {} ({} file(s))",
        base.display(),
        manifest.entries.len()
    ));
    Ok(())
}
fn snapshots(name: &str) -> Result<Vec<PathBuf>, String> {
    let dir = backups_for(name)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = Vec::new();
    for item in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = item.map_err(|e| e.to_string())?.path();
        if path.is_dir() && path.join("manifest.toml").is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    paths.reverse();
    Ok(paths)
}
pub fn list(name: &str) -> Result<(), String> {
    output::banner();
    output::heading(&format!("Backups: {name}"));
    let items = snapshots(name)?;
    if items.is_empty() {
        output::warn("No backups found");
    }
    for path in items {
        println!(
            "  {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    Ok(())
}
pub(crate) fn selected_folder(name: &str, selected: Option<&str>) -> Result<PathBuf, String> {
    let items = snapshots(name)?;
    if let Some(id) = selected {
        config::valid_name(id)?;
        items
            .into_iter()
            .find(|p| p.file_name().is_some_and(|s| s == id))
            .ok_or("Backup ID not found".into())
    } else {
        items.into_iter().next().ok_or("No backup found".into())
    }
}
fn read_manifest(folder: &Path, name: &str) -> Result<Manifest, String> {
    let manifest: Manifest = toml::from_str(
        &fs::read_to_string(folder.join("manifest.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.profile != name {
        return Err("Backup profile mismatch".into());
    }
    Ok(manifest)
}
fn build_actions(
    folder: &Path,
    profile: &Profile,
    manifest: &Manifest,
) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut actions = Vec::new();
    let mut seen = BTreeSet::new();
    if !manifest.entries.is_empty() {
        for e in &manifest.entries {
            config::valid_name(&e.alias)?;
            let rel = PathBuf::from(&e.relative);
            if !clean_relative(&rel)
                || rel.components().next().unwrap().as_os_str() != std::ffi::OsStr::new(&e.alias)
            {
                return Err("Unsafe backup entry".into());
            }
            let src = folder.join("data").join(&rel);
            if !src.is_file()
                || fs::symlink_metadata(&src)
                    .map_err(|x| x.to_string())?
                    .file_type()
                    .is_symlink()
            {
                return Err(format!("Invalid backup file: {}", src.display()));
            }
            if hash(&src)? != e.sha256 {
                return Err(format!("SHA-256 mismatch: {}", src.display()));
            }
            let target = config::expand_path(
                profile
                    .files
                    .get(&e.alias)
                    .ok_or("Alias missing from profile")?,
            )?;
            let tail = rel.strip_prefix(&e.alias).map_err(|e| e.to_string())?;
            let dest = if tail == Path::new("__file__") {
                target
            } else {
                target.join(tail)
            };
            if !seen.insert(dest.clone()) {
                return Err("Duplicate destination".into());
            }
            actions.push((src, dest));
        }
    } else {
        // Read-only compatibility with backups made by ConfigSync v0.1.0.
        for (alias, filename) in &manifest.files {
            config::valid_name(alias)?;
            if filename != &format!("{alias}.bak") {
                return Err("Unsafe legacy entry".into());
            }
            let src = folder.join(filename);
            if !src.is_file() {
                return Err(format!("Missing backup: {}", src.display()));
            }
            let dest =
                config::expand_path(profile.files.get(alias).ok_or("Legacy alias missing")?)?;
            if !seen.insert(dest.clone()) {
                return Err("Duplicate destination".into());
            }
            actions.push((src, dest));
        }
    }
    Ok(actions)
}
fn check_dest(dest: &Path) -> Result<(), String> {
    for path in dest.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return Err(format!("Symlink destination rejected: {}", path.display()));
            }
            if path == dest && !meta.is_file() {
                return Err(format!("Destination is not a file: {}", dest.display()));
            }
        }
    }
    Ok(())
}
pub fn restore(
    name: &str,
    profile: &Profile,
    selected: Option<&str>,
    yes: bool,
    dry_run: bool,
) -> Result<(), String> {
    if yes && dry_run {
        return Err("Choose --yes or --dry-run, not both".into());
    }
    output::banner();
    output::heading(&format!("Restore: {name}"));
    let folder = selected_folder(name, selected)?;
    let manifest = read_manifest(&folder, name)?;
    let actions = build_actions(&folder, profile, &manifest)?;
    output::info(&format!(
        "Backup ID: {}",
        folder.file_name().unwrap_or_default().to_string_lossy()
    ));
    for (_, dest) in &actions {
        check_dest(dest)?;
        output::step(&format!("Restore into: {}", dest.display()));
    }
    if !yes || dry_run {
        output::warn("Nothing restored. Use --yes to overwrite destination files.");
        return Ok(());
    }
    // Recoverable copies are kept in a timestamped directory. Never write onto them.
    let recovery = config::data_dir()?.join("recovery").join(format!(
        "{}-{}",
        name,
        Utc::now().format("%Y%m%dT%H%M%S%.fZ")
    ));
    fs::create_dir_all(&recovery).map_err(|e| e.to_string())?;
    for (index, (_, dest)) in actions.iter().enumerate() {
        if dest.exists() {
            let original = recovery.join(format!("{index}.original"));
            fs::copy(dest, &original)
                .map_err(|e| format!("Cannot preserve {}: {e}", dest.display()))?;
        }
    }
    // After all original files are preserved, replace one by one.
    for (index, (src, dest)) in actions.iter().enumerate() {
        let parent = dest.parent().ok_or("Invalid destination")?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        check_dest(dest)?;
        let staging = parent.join(format!(".csync-stage-{}-{index}", std::process::id()));
        if staging.exists() {
            return Err(format!("Staging path exists: {}", staging.display()));
        }
        fs::copy(src, &staging).map_err(|e| format!("Cannot stage: {e}"))?;
        if dest.exists() {
            fs::remove_file(dest).map_err(|e| format!("Cannot replace {}: {e}", dest.display()))?;
        }
        fs::rename(&staging, dest).map_err(|e| {
            format!(
                "Restore stopped: {e}. Recovery copies: {}",
                recovery.display()
            )
        })?;
    }
    output::success(&format!("Restored {} file(s)", actions.len()));
    output::info(&format!(
        "Previous files preserved at: {}",
        recovery.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prevent_traversal() {
        assert!(!clean_relative(Path::new("../secret")));
        assert!(!clean_relative(Path::new("/tmp/secret")));
        assert!(clean_relative(Path::new("folder/settings.json")));
    }
}

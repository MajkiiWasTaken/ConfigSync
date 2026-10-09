/************************************************
* File: comparison.rs
* Author: Michal Švrček
*
* Profile inventory, backup size and SHA-256 comparison
*
* ver. 0.5.0
*************************************************/
use crate::{
    backup,
    config::{self, Profile},
    output,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

fn hash(path: &Path) -> Result<String, String> {
    let mut source = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut sha = Sha256::new();
    let mut buffer = [0u8; 32768];
    loop {
        let len = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if len == 0 {
            break;
        }
        sha.update(&buffer[..len]);
    }
    Ok(format!("{:x}", sha.finalize()))
}
fn files(root: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let meta = fs::symlink_metadata(root).map_err(|e| format!("{}: {e}", root.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("Symlink not supported: {}", root.display()));
    }
    if meta.is_file() {
        return Ok(vec![(PathBuf::from("__file__"), root.to_path_buf())]);
    }
    if !meta.is_dir() {
        return Err(format!("Not a file or directory: {}", root.display()));
    }
    let mut out = Vec::new();
    for item in WalkDir::new(root).follow_links(false) {
        let item = item.map_err(|e| e.to_string())?;
        if item.file_type().is_symlink() {
            return Err(format!("Symlink found: {}", item.path().display()));
        }
        if item.file_type().is_file() {
            let relative = item.path().strip_prefix(root).map_err(|e| e.to_string())?;
            out.push((relative.to_path_buf(), item.path().to_path_buf()));
        }
    }
    Ok(out)
}
pub fn show(name: &str, profile: &Profile) -> Result<(), String> {
    output::banner();
    output::heading(&format!("Profile details: {name}"));
    for (alias, raw) in &profile.files {
        let path = config::expand_path(raw)?;
        match files(&path) {
            Ok(entries) => {
                let mut total = 0u64;
                for (_, path) in &entries {
                    total += fs::metadata(path).map_err(|e| e.to_string())?.len();
                }
                output::info(&format!(
                    "{alias}: {} | {} file(s) | {} bytes",
                    path.display(),
                    entries.len(),
                    total
                ));
            }
            Err(e) => output::warn(&format!("{alias}: {e}")),
        }
    }
    Ok(())
}
fn safe_entry(path: &str, alias: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && p.components()
            .next()
            .is_some_and(|c| c.as_os_str() == alias)
}
pub fn diff(name: &str, profile: &Profile, id: Option<&str>) -> Result<(), String> {
    let folder = backup::selected_folder(name, id)?;
    let manifest: backup::Manifest = toml::from_str(
        &fs::read_to_string(folder.join("manifest.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.profile != name {
        return Err("Backup profile mismatch".into());
    }
    if manifest.entries.is_empty() {
        return Err("Diff requires a v0.2+ backup with SHA-256 entries".into());
    }
    output::banner();
    output::heading(&format!("Diff: {name}"));
    let mut old: BTreeMap<String, String> = BTreeMap::new();
    for entry in manifest.entries {
        if !safe_entry(&entry.relative, &entry.alias) {
            return Err("Unsafe entry in backup manifest".into());
        }
        let source = folder.join("data").join(&entry.relative);
        if fs::symlink_metadata(&source)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Symlink in backup".into());
        }
        if hash(&source)? != entry.sha256 {
            return Err(format!("Backup integrity mismatch: {}", entry.relative));
        }
        old.insert(entry.relative, entry.sha256);
    }
    let mut current = BTreeMap::new();
    for (alias, raw) in &profile.files {
        let root = config::expand_path(raw)?;
        if !root.exists() {
            output::warn(&format!("Missing current source: {}", root.display()));
            continue;
        }
        for (rel, path) in files(&root)? {
            let label = PathBuf::from(alias)
                .join(rel)
                .to_string_lossy()
                .replace('\\', "/");
            current.insert(label, hash(&path)?);
        }
    }
    let keys: BTreeSet<_> = old.keys().chain(current.keys()).cloned().collect();
    let (mut added, mut modified, mut removed, mut unchanged) = (0, 0, 0, 0);
    for key in keys {
        match (old.get(&key), current.get(&key)) {
            (None, Some(_)) => {
                added += 1;
                output::success(&format!("ADDED    {key}"));
            }
            (Some(_), None) => {
                removed += 1;
                output::warn(&format!("MISSING  {key}"));
            }
            (Some(a), Some(b)) if a != b => {
                modified += 1;
                output::step(&format!("MODIFIED {key}"));
            }
            _ => {
                unchanged += 1;
            }
        }
    }
    output::info(&format!(
        "Summary: {added} added, {modified} modified, {removed} missing, {unchanged} unchanged"
    ));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_confined() {
        assert!(safe_entry("gitconfig/__file__", "gitconfig"));
        assert!(!safe_entry("../outside", "gitconfig"));
        assert!(!safe_entry("other/file", "gitconfig"));
    }
}

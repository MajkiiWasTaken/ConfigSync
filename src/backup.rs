/************************************************
* File: backup.rs
* Author: Michal Švrček
*
* Backup and restore file-based configuration profiles
*
* ver. 0.1.0
*************************************************/

use crate::{
    config::{self, Profile},
    output,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub profile: String,
    pub created: String,
    pub files: BTreeMap<String, String>,
}

fn backups_for(name: &str) -> Result<PathBuf, String> {
    config::valid_name(name)?;
    Ok(config::data_dir()?.join(name))
}

pub fn create(name: &str, profile: &Profile, dry_run: bool) -> Result<(), String> {
    output::banner();
    output::heading(&format!("Backup: {name}"));
    let mut entries = Vec::new();
    for (alias, raw_path) in &profile.files {
        let path = config::expand_path(raw_path)?;
        if !path.is_file() {
            return Err(format!("Not a file or does not exist: {}", path.display()));
        }
        entries.push((alias.clone(), path));
    }
    for (alias, path) in &entries {
        output::step(&format!("{alias}: {}", path.display()));
    }
    if dry_run {
        output::warn("DRY RUN: No backup created");
        return Ok(());
    }

    let created = Utc::now();
    let suffix = created.format("%Y%m%dT%H%M%S%.fZ").to_string();
    let folder = backups_for(name)?.join(suffix);
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut manifest = Manifest {
        profile: name.into(),
        created: created.to_rfc3339(),
        files: BTreeMap::new(),
    };
    for (alias, path) in &entries {
        let filename = format!("{alias}.bak");
        if let Err(e) = fs::copy(path, folder.join(&filename)) {
            let _ = fs::remove_dir_all(&folder);
            return Err(e.to_string());
        }
        manifest.files.insert(alias.clone(), filename);
    }
    let text = toml::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    if let Err(e) = fs::write(folder.join("manifest.toml"), text) {
        let _ = fs::remove_dir_all(&folder);
        return Err(e.to_string());
    }
    output::success(&format!("Backup created: {}", folder.display()));
    Ok(())
}

fn snapshots(name: &str) -> Result<Vec<PathBuf>, String> {
    let dir = backups_for(name)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
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
        return Ok(());
    }
    for path in items {
        println!(
            "  {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
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
    output::banner();
    output::heading(&format!("Restore: {name}"));
    if yes && dry_run {
        return Err("Choose --yes or --dry-run, not both".into());
    }
    let snapshots = snapshots(name)?;
    let folder = if let Some(id) = selected {
        config::valid_name(id)?;
        snapshots
            .into_iter()
            .find(|p| p.file_name().is_some_and(|x| x == id))
            .ok_or("Backup ID not found")?
    } else {
        snapshots.into_iter().next().ok_or("No backup found")?
    };
    let manifest: Manifest = toml::from_str(
        &fs::read_to_string(folder.join("manifest.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.profile != name {
        return Err("Backup profile mismatch".into());
    }
    let mut actions = Vec::new();
    for (alias, filename) in &manifest.files {
        config::valid_name(alias)?;
        if filename != &format!("{alias}.bak") {
            return Err(format!("Invalid backup entry: {alias}"));
        }
        let source = folder.join(filename);
        if !source.is_file() {
            return Err(format!("Missing backup file: {}", source.display()));
        }
        let destination = profile
            .files
            .get(alias)
            .ok_or_else(|| format!("Destination for '{alias}' is not configured"))?;
        let destination = config::expand_path(destination)?;
        if !destination.parent().is_some_and(Path::is_dir) {
            return Err(format!(
                "Destination directory missing: {}",
                destination.display()
            ));
        }
        actions.push((source, destination));
    }
    output::info(&format!(
        "Backup ID: {}",
        folder.file_name().unwrap_or_default().to_string_lossy()
    ));
    for (_, destination) in &actions {
        output::step(&format!("Restore into: {}", destination.display()));
    }
    if dry_run || !yes {
        output::warn("Nothing restored. Use --yes to overwrite destination files.");
        return Ok(());
    }
    // Stage restoration files before touching any existing destination.
    let mut staged = Vec::new();
    for (index, (source, dest)) in actions.iter().enumerate() {
        let temp = dest.with_extension(format!("csync-stage-{}-{index}", std::process::id()));
        if let Err(err) = fs::copy(source, &temp) {
            for p in &staged {
                let _ = fs::remove_file(p);
            }
            return Err(format!("Staging failed: {err}"));
        }
        staged.push(temp);
    }
    for (temp, (_, dest)) in staged.iter().zip(&actions) {
        // A pre-existing file is explicitly replaced only after --yes.
        if dest.exists() {
            fs::remove_file(dest).map_err(|e| format!("Cannot replace {}: {e}", dest.display()))?;
        }
        fs::rename(temp, dest)
            .map_err(|e| format!("Restore interrupted for {}: {e}", dest.display()))?;
    }
    output::success(&format!("Restored {} file(s)", actions.len()));
    Ok(())
}

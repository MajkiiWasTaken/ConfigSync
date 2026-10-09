/************************************************
* File: migration.rs
* Author: Michal Švrček
*
* Migration planning, conflict detection and guarded restore
*
* ver. 0.7.0
*************************************************/

use crate::{backup, config::Profile, output};
use std::{collections::BTreeSet, fs, path::Path};

fn destination_state(path: &Path) -> Result<&'static str, String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "Symbolic link in destination: {}",
                    ancestor.display()
                ));
            }
            Ok(meta) if ancestor == path && meta.is_file() => return Ok("OVERWRITE"),
            Ok(meta) if ancestor == path => {
                return Err(format!(
                    "Destination is not a regular file: {}",
                    path.display()
                ));
            }
            Ok(meta) if ancestor != path && !meta.is_dir() => {
                return Err(format!("Parent is not a directory: {}", ancestor.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", ancestor.display())),
        }
    }
    Ok("NEW")
}

pub fn run(
    name: &str,
    profile: &Profile,
    id: Option<&str>,
    yes: bool,
    only: &[String],
    skip_existing: bool,
) -> Result<(), String> {
    output::banner();
    output::heading(&format!("Migration plan: {name}"));
    let mut plan = backup::restore_plan(name, profile, id)?;
    let allowed: BTreeSet<String> = only.iter().cloned().collect();
    for alias in &allowed {
        if !profile.files.contains_key(alias) {
            return Err(format!("Unknown alias: {alias}"));
        }
    }
    if !allowed.is_empty() {
        let folder = backup::selected_folder(name, id)?;
        let manifest: backup::Manifest = toml::from_str(
            &fs::read_to_string(folder.join("manifest.toml")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if manifest.entries.is_empty() {
            return Err("Selective migration requires a v0.2+ snapshot".into());
        }
        let chosen: BTreeSet<_> = manifest
            .entries
            .iter()
            .filter(|entry| allowed.contains(&entry.alias))
            .map(|entry| folder.join("data").join(&entry.relative))
            .collect();
        plan.retain(|(src, _)| chosen.contains(src));
    }
    if skip_existing {
        plan.retain(|(_, dest)| !dest.exists());
    }
    if plan.is_empty() {
        return Err("No files matched the migration options".into());
    }
    let mut seen = BTreeSet::new();
    let mut overwrites = 0;
    let mut created = 0;
    for (_, destination) in &plan {
        if !seen.insert(destination.clone()) {
            return Err(format!("Duplicate destination: {}", destination.display()));
        }
        match destination_state(destination)? {
            "OVERWRITE" => {
                overwrites += 1;
                output::warn(&format!("OVERWRITE  {}", destination.display()));
            }
            _ => {
                created += 1;
                output::info(&format!("NEW        {}", destination.display()));
            }
        }
    }
    output::info(&format!(
        "{} new file(s), {} existing file(s) to overwrite",
        created, overwrites
    ));
    if !yes {
        output::warn("Preview only. Review destination paths; use --yes to restore.");
        return Ok(());
    }
    output::warn("Applying migration using recovery backups from the restore engine.");
    backup::restore_filtered(
        name,
        profile,
        id,
        true,
        false,
        if allowed.is_empty() {
            None
        } else {
            Some(&allowed)
        },
        skip_existing,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_destination_is_new() {
        let path = std::env::temp_dir().join(format!("csync-nonexistent-{}", std::process::id()));
        assert_eq!(destination_state(&path).unwrap(), "NEW");
    }
    #[test]
    fn existing_regular_file_is_overwrite() {
        let file = std::env::current_exe().unwrap();
        assert_eq!(destination_state(&file).unwrap(), "OVERWRITE");
    }
}

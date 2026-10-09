/************************************************
* File: profile_manager.rs
* Author: Michal Švrček
*
* Initialize and manage selected configuration profiles
*
* ver. 0.7.0
*************************************************/

use crate::{
    config::{self, Config, Profile},
    output, ProfileCommands,
};
use std::{collections::BTreeMap, fs, path::Path};

fn write_config(path: &Path, data: &Config) -> Result<(), String> {
    let serialized = toml::to_string_pretty(data).map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("Invalid configuration path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension("toml.tmp");
    if temp.exists() {
        return Err(format!("Temporary config file exists: {}", temp.display()));
    }
    fs::write(&temp, serialized).map_err(|e| e.to_string())?;
    // Do not overwrite an existing config via rename without keeping a recovery copy.
    if path.exists() {
        let previous = path.with_extension("toml.bak");
        if previous.exists() {
            fs::remove_file(&previous).map_err(|e| e.to_string())?;
        }
        fs::copy(path, &previous).map_err(|e| e.to_string())?;
    }
    if path.exists() {
        // Windows generally cannot rename onto an existing file. Keep the
        // original until a complete replacement has been written.
        fs::copy(&temp, path).map_err(|e| format!("Cannot replace config: {e}"))?;
        fs::remove_file(&temp).map_err(|e| e.to_string())?;
    } else {
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn init(custom: Option<&Path>, scan: bool) -> Result<(), String> {
    let path = custom
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(config::default_config_path)?;
    if path.exists() {
        return Err(format!(
            "Configuration already exists: {} (not overwritten)",
            path.display()
        ));
    }
    let mut settings = Config {
        profiles: BTreeMap::new(),
    };
    if scan {
        let mut files = BTreeMap::new();
        for (label, location) in crate::discovery::candidates()? {
            if !location.exists() {
                continue;
            }
            let alias = match label.as_str() {
                "Git configuration" => "gitconfig",
                "VS Code settings" => "vscode_settings",
                "VS Code keybindings" => "vscode_keybindings",
                "Windows Terminal settings" => "windows_terminal",
                "PowerShell profile" => "powershell_profile",
                "DevDock sessions" => "devdock_sessions",
                "DeployTool configuration" => "deploytool_config",
                _ => continue,
            };
            // Avoid implicitly adding directories and files whose names suggest secrets.
            files.insert(alias.to_owned(), location.to_string_lossy().into_owned());
        }
        settings.profiles.insert(
            "development".into(),
            Profile {
                description: Some("Discovered development configuration".into()),
                files,
            },
        );
    }
    write_config(&path, &settings)?;
    output::banner();
    output::success(&format!("Created configuration: {}", path.display()));
    output::info(if scan {
        "Review the discovered paths before backup: csync profile-show development"
    } else {
        "Next: csync profile add development"
    });
    Ok(())
}

pub fn run(path: &Path, command: &ProfileCommands) -> Result<(), String> {
    let mut settings = config::load(path)?;
    match command {
        ProfileCommands::Add { name, description } => {
            config::valid_name(name)?;
            if settings.profiles.contains_key(name) {
                return Err(format!("Profile already exists: {name}"));
            }
            settings.profiles.insert(
                name.clone(),
                Profile {
                    description: description.clone(),
                    files: BTreeMap::new(),
                },
            );
        }
        ProfileCommands::Set {
            name,
            alias,
            path: raw,
        } => {
            config::valid_name(alias)?;
            config::expand_path(raw)?;
            let profile = settings
                .profiles
                .get_mut(name)
                .ok_or_else(|| format!("Unknown profile: {name}"))?;
            if profile.files.contains_key(alias) {
                return Err(format!(
                    "Alias '{alias}' already exists. Unset it before changing it."
                ));
            }
            let expanded = config::expand_path(raw)?;
            if !expanded.exists() {
                return Err(format!("Path does not exist: {}", expanded.display()));
            }
            if fs::symlink_metadata(&expanded)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("Symbolic links are not supported".into());
            }
            profile.files.insert(alias.clone(), raw.clone());
        }
        ProfileCommands::Unset { name, alias } => {
            let profile = settings
                .profiles
                .get_mut(name)
                .ok_or_else(|| format!("Unknown profile: {name}"))?;
            if profile.files.remove(alias).is_none() {
                return Err(format!("Unknown alias: {alias}"));
            }
        }
        ProfileCommands::Remove { name, yes } => {
            if !yes {
                return Err("Removing a profile requires --yes; backups will be preserved".into());
            }
            if settings.profiles.remove(name).is_none() {
                return Err(format!("Unknown profile: {name}"));
            }
        }
    }
    write_config(path, &settings)?;
    output::banner();
    output::success(&format!("Updated configuration: {}", path.display()));
    Ok(())
}

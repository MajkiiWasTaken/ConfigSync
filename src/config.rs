/************************************************
* File: config.rs
* Author: Michal Švrček
*
* Profile configuration and local storage paths
*
* ver. 0.7.0
*************************************************/

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub profiles: BTreeMap<String, Profile>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Profile {
    pub description: Option<String>,
    pub files: BTreeMap<String, String>,
}

pub fn user_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        Ok(PathBuf::from(env::var_os("APPDATA").ok_or("APPDATA is missing")?).join("ConfigSync"))
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from(
            env::var_os("XDG_CONFIG_HOME")
                .or_else(|| {
                    env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").into_os_string())
                })
                .ok_or("HOME is missing")?,
        )
        .join("configsync"))
    }
}
pub fn data_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        Ok(
            PathBuf::from(env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is missing")?)
                .join("ConfigSync")
                .join("backups"),
        )
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from(
            env::var_os("XDG_DATA_HOME")
                .or_else(|| {
                    env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".local/share").into_os_string())
                })
                .ok_or("HOME is missing")?,
        )
        .join("configsync/backups"))
    }
}
pub fn default_config_path() -> Result<PathBuf, String> {
    Ok(user_dir()?.join("config.toml"))
}
pub fn load(path: &Path) -> Result<Config, String> {
    let data =
        fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let config: Config = toml::from_str(&data).map_err(|e| format!("Invalid TOML: {e}"))?;

    for (name, profile) in &config.profiles {
        valid_name(name)?;

        for key in profile.files.keys() {
            valid_name(key)?;
        }
    }
    Ok(config)
}
pub fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Err(format!(
            "Invalid name '{name}' (use letters, digits, _ and - only)"
        ))
    } else {
        Ok(())
    }
}
pub fn expand_path(input: &str) -> Result<PathBuf, String> {
    let mut expanded = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let mut key = String::new();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == '}' {
                    closed = true;
                    break;
                }
                key.push(ch);
            }
            if !closed
                || key.is_empty()
                || !key.chars().all(|x| x.is_ascii_alphanumeric() || x == '_')
            {
                return Err(format!("Invalid variable in path: {input}"));
            }
            let value =
                env::var(&key).map_err(|_| format!("Environment variable {key} is not set"))?;
            expanded.push_str(&value);
        } else {
            expanded.push(c);
        }
    }
    if expanded == "~" || expanded.starts_with("~/") || expanded.starts_with("~\\") {
        let home = env::var("USERPROFILE")
            .or_else(|_| env::var("HOME"))
            .map_err(|_| "Home directory unknown")?;
        expanded = format!("{}{}", home, &expanded[1..]);
    }
    let path = PathBuf::from(expanded);
    if !path.is_absolute() {
        return Err(format!("Source path must be absolute: {input}"));
    }
    Ok(path)
}

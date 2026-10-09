/************************************************
* File: discovery.rs
* Author: Michal Švrček
*
* Detect common developer configuration paths
*
* ver. 0.5.0
*************************************************/
use crate::{config, output};
use std::{env, path::PathBuf};

pub fn candidates() -> Result<Vec<(String, PathBuf)>, String> {
    let home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .ok_or("Home directory unavailable")?;
    let home = PathBuf::from(home);
    let mut candidates: Vec<(&str, PathBuf)> = vec![("Git configuration", home.join(".gitconfig"))];
    #[cfg(windows)]
    if let Some(appdata) = env::var_os("APPDATA") {
        let appdata = PathBuf::from(appdata);
        candidates.extend([
            ("VS Code settings", appdata.join("Code/User/settings.json")),
            (
                "VS Code keybindings",
                appdata.join("Code/User/keybindings.json"),
            ),
            (
                "Windows Terminal settings",
                appdata.join("Microsoft/Windows Terminal/settings.json"),
            ),
            (
                "PowerShell profile",
                home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
            ),
            ("DevDock sessions", appdata.join("DevDock")),
            (
                "DeployTool configuration",
                appdata.join("DeployTool/deploy.toml"),
            ),
        ]);
    }
    #[cfg(not(windows))]
    candidates.extend([
        (
            "VS Code settings",
            home.join(".config/Code/User/settings.json"),
        ),
        (
            "PowerShell profile",
            home.join(".config/powershell/Microsoft.PowerShell_profile.ps1"),
        ),
        ("DevDock sessions", home.join(".config/devdock")),
        (
            "DeployTool configuration",
            home.join(".config/deploytool/deploy.toml"),
        ),
    ]);
    Ok(candidates
        .into_iter()
        .map(|(label, path)| (label.to_owned(), path))
        .collect())
}

pub fn scan() -> Result<(), String> {
    output::banner();
    output::heading("Scanning development environment");
    let mut found = 0;
    let candidates = candidates()?;
    for (label, path) in candidates {
        if path.exists() {
            output::success(&format!("{label}: {}", path.display()));
            found += 1;
        } else {
            output::warn(&format!("Not found: {label}"));
        }
    }
    output::warn("SSH keys, credentials and tokens are not scanned or copied.");
    output::info(&format!("Found {found} configuration location(s)."));
    output::info(&format!(
        "Use csync init --scan to create a new discovered profile, or edit {}.",
        config::default_config_path()?.display()
    ));
    Ok(())
}

/************************************************
* File: main.rs
* Author: Michal Švrček
*
* ConfigSync CLI entry point
*
* ver. 0.1.0
*************************************************/

mod backup;
mod config;
mod output;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "csync",
    version,
    about = "Developer configuration backup manager"
)]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show configured profiles
    Profiles,
    /// Show profile details
    Info { name: String },
    /// Back up files in a profile
    Backup {
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// List saved backups for a profile
    List { name: String },
    /// Restore from latest or selected backup (preview by default)
    Restore {
        name: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Show configuration and backups directory
    Paths,
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    if matches!(cli.command, Commands::Paths) {
        output::banner();
        output::info(&format!(
            "Config: {}",
            cli.config
                .unwrap_or(config::default_config_path()?)
                .display()
        ));
        output::info(&format!("Backups: {}", config::data_dir()?.display()));
        return Ok(());
    }
    let config_path = cli.config.unwrap_or(config::default_config_path()?);
    let settings = config::load(&config_path)?;
    match cli.command {
        Commands::Profiles => {
            output::banner();
            output::heading("Configured profiles");
            for (name, profile) in &settings.profiles {
                println!("  {name}  {}", profile.description.as_deref().unwrap_or(""));
            }
            output::success(&format!("{} profile(s)", settings.profiles.len()));
        }
        Commands::Info { name } => {
            let profile = settings
                .profiles
                .get(&name)
                .ok_or(format!("Unknown profile: {name}"))?;
            output::banner();
            output::heading(&format!("Profile: {name}"));
            if let Some(description) = &profile.description {
                output::info(description);
            }
            for (alias, raw) in &profile.files {
                output::step(&format!("{alias}: {}", config::expand_path(raw)?.display()));
            }
        }
        Commands::Backup { name, dry_run } => backup::create(
            &name,
            settings
                .profiles
                .get(&name)
                .ok_or(format!("Unknown profile: {name}"))?,
            dry_run,
        )?,
        Commands::List { name } => {
            if !settings.profiles.contains_key(&name) {
                return Err(format!("Unknown profile: {name}"));
            }
            backup::list(&name)?;
        }
        Commands::Restore {
            name,
            id,
            yes,
            dry_run,
        } => backup::restore(
            &name,
            settings
                .profiles
                .get(&name)
                .ok_or(format!("Unknown profile: {name}"))?,
            id.as_deref(),
            yes,
            dry_run,
        )?,
        Commands::Paths => unreachable!(),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        output::error(&error);
        std::process::exit(1);
    }
}

/************************************************
* File: main.rs
* Author: Michal Švrček
*
* ConfigSync CLI entry point
*
* ver. 0.5.0
*************************************************/

mod archive;
mod backup;
mod comparison;
mod config;
mod discovery;
mod output;
mod profile_manager;

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
    /// Initialize a user configuration (never overwrites)
    Init {
        #[arg(long)]
        scan: bool,
    },
    /// Manage profiles and their selected paths
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
    /// Find common configuration locations without modifying files
    Scan,
    /// Show configured profiles
    Profiles,
    /// Show profile details
    Info { name: String },
    /// Detailed profile inventory
    ProfileShow { name: String },
    /// Compare current configuration with last backup
    Diff {
        name: String,
        #[arg(long)]
        id: Option<String>,
    },
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
    /// Export an existing backup as a portable ZIP
    Export {
        name: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Import an existing ZIP as a local backup (does not restore)
    Import { file: PathBuf },
    /// Show configuration and backups directory
    Paths,
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// Create an empty profile
    Add {
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Add a file or directory to a profile
    Set {
        name: String,
        alias: String,
        path: String,
    },
    /// Remove one path from a profile
    Unset { name: String, alias: String },
    /// Remove a profile, leaving its backups untouched
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    if let Commands::Init { scan } = &cli.command {
        return profile_manager::init(cli.config.as_deref(), *scan);
    }
    if let Commands::Profile { command } = &cli.command {
        let path = cli.config.clone().unwrap_or(config::default_config_path()?);
        return profile_manager::run(&path, command);
    }
    if matches!(cli.command, Commands::Import { .. }) {
        if let Commands::Import { file } = cli.command {
            output::banner();
            archive::import(&file)?;
        }
        return Ok(());
    }
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
        Commands::Scan => discovery::scan()?,
        Commands::Profiles => {
            output::banner();
            output::heading("Configured profiles");
            for (name, profile) in &settings.profiles {
                println!("  {name}  {}", profile.description.as_deref().unwrap_or(""));
            }
            output::success(&format!("{} profile(s)", settings.profiles.len()));
        }
        Commands::ProfileShow { name } => comparison::show(
            &name,
            settings
                .profiles
                .get(&name)
                .ok_or(format!("Unknown profile: {name}"))?,
        )?,
        Commands::Diff { name, id } => comparison::diff(
            &name,
            settings
                .profiles
                .get(&name)
                .ok_or(format!("Unknown profile: {name}"))?,
            id.as_deref(),
        )?,
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
        Commands::Export { name, id, output } => {
            if !settings.profiles.contains_key(&name) {
                return Err(format!("Unknown profile: {name}"));
            }
            output::banner();
            archive::export(&name, id.as_deref(), &output)?;
        }
        Commands::Import { .. }
        | Commands::Paths
        | Commands::Init { .. }
        | Commands::Profile { .. } => unreachable!(),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        output::error(&error);
        std::process::exit(1);
    }
}

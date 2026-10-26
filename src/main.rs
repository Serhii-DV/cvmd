mod application;
mod cli;
mod config;
mod info;
mod pdf;
mod selection;
mod workspace;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = config::location(cli.config.as_deref())?;
    match cli.command {
        Commands::Info => info::show(&config_path, cli.config.is_some())?,
        Commands::Init { path, master } => {
            let root = workspace::initialize(&path, master.as_deref(), &config_path)?;
            println!("Initialized {}", root.display());
            println!("Configuration: {}", config_path.display());
        }
        Commands::New {
            company,
            position,
            job,
        } => {
            let config = config::Config::load(&config_path)?;
            let path = application::create(&config, &company, &position, job.as_deref())?;
            // Print just the path so it can be used with: cd "$(cvmd new ...)".
            println!("{}", path.display());
        }
        Commands::Edit { alias } => {
            let config = config::Config::load(&config_path)?;
            let path = selection::resolve(&config, alias.as_deref())?;
            application::edit(&config, &path)?;
        }
        Commands::Build { alias } => {
            let config = config::Config::load(&config_path)?;
            let path = selection::resolve(&config, alias.as_deref())?;
            let output = pdf::build(&config, &path)?;
            println!("Built {}", output.display());
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

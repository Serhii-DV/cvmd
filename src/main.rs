mod application;
mod cli;
mod config;
mod config_command;
mod info;
mod pdf;
mod selection;
mod workspace;

use anyhow::{Result, ensure};
use clap::Parser;
use cli::{Cli, Commands};

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = || config::location(cli.config.as_deref());
    match cli.command {
        Commands::Info => {
            let location = config_path()?;
            info::show(&location.path, location.source)?;
        }
        Commands::Config(args) => {
            ensure!(
                !args.local || cli.config.is_none(),
                "--local cannot be used with --config"
            );
            if args.key.is_none() && !args.list {
                return cli::print_command_help("config");
            }
            let location = if args.local {
                config::local_location()?
            } else {
                config_path()?
            };
            config_command::run(&location.path, &args)?;
        }
        Commands::Init { path, source } => {
            let config_path = config::initial_location(&path, cli.config.as_deref())?;
            let root = workspace::initialize(&path, source.as_deref(), &config_path)?;
            println!("Initialized {}", root.display());
            println!("Configuration: {}", config_path.display());
        }
        Commands::New {
            company,
            position,
            job,
        } => {
            let config = config::Config::load(&config_path()?.path)?;
            let path = application::create(&config, &company, &position, job.as_deref())?;
            // Print just the path so it can be used with: cd "$(cvmd new ...)".
            println!("{}", path.display());
        }
        Commands::Show { alias } => {
            let Some(alias) = alias else {
                return cli::print_command_help("show");
            };
            let config = config::Config::load(&config_path()?.path)?;
            let path = selection::resolve_directory(&config, &alias)?;
            application::show(&path)?;
        }
        Commands::Open { alias } => {
            let Some(alias) = alias else {
                return cli::print_command_help("open");
            };
            let config = config::Config::load(&config_path()?.path)?;
            let path = selection::resolve_directory(&config, &alias)?;
            pdf::open(&config, &path, &alias)?;
        }
        Commands::Edit { alias } => {
            let Some(alias) = alias else {
                return cli::print_command_help("edit");
            };
            let config = config::Config::load(&config_path()?.path)?;
            let path = selection::resolve(&config, &alias)?;
            application::edit(&config, &path)?;
        }
        Commands::Build { alias, verbose } => {
            let Some(alias) = alias else {
                return cli::print_command_help("build");
            };
            let location = config_path()?;
            if verbose {
                eprintln!("Config: {} [{}]", location.path.display(), location.source);
            }
            let config = config::Config::load(&location.path)?;
            let path = selection::resolve(&config, &alias)?;
            let output = pdf::build(&config, &path, verbose)?;
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

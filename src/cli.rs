use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Use an alternate user configuration file
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show the active configuration, workspace, tools and current application
    Info,
    /// Initialize a CV workspace and save its location
    Init {
        path: PathBuf,
        /// Import an existing Markdown CV instead of the starter template
        #[arg(long, value_name = "FILE")]
        master: Option<PathBuf>,
    },
    /// Copy the master CV into a new dated application directory
    New {
        company: String,
        position: String,
        /// Copy an existing job description into job.md
        #[arg(long, value_name = "FILE")]
        job: Option<PathBuf>,
    },
    /// Open an application CV by company or position alias
    #[command(after_help = "Examples:\n  cvmd edit google\n  cvmd edit software_engineer")]
    Edit {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
    },
    /// Render an application CV by company or position alias
    #[command(after_help = "Examples:\n  cvmd build google\n  cvmd build software_engineer")]
    Build {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
    },
}

pub fn print_command_help(name: &str) -> Result<()> {
    let mut cli = Cli::command();
    cli.build();
    cli.find_subcommand_mut(name)
        .context("Cannot find command help")?
        .print_help()?;
    println!();
    Ok(())
}

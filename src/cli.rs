use clap::{Parser, Subcommand};
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
    /// Open an application CV, optionally finding it by company or position alias
    Edit {
        /// Exact company or position alias; choose from a list if several match
        alias: Option<String>,
    },
    /// Render an application CV, optionally finding it by company or position alias
    Build {
        /// Exact company or position alias; choose from a list if several match
        alias: Option<String>,
    },
}

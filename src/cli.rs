use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Use an explicit configuration file instead of workspace discovery
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show the active configuration, workspace, tools and current application
    Info,
    /// Read or change configuration values
    #[command(
        after_help = "Examples:\n  cvmd config source\n  cvmd config --local source ~/Documents/my-cv.md\n  cvmd config pdf.renderer md-to-pdf\n  cvmd config pdf.filename 'My CV.pdf'\n  cvmd config --list\n  cvmd config --unset editor\n\nKeys: root, source, editor, browser, pdf.renderer, pdf.config_file, pdf.filename"
    )]
    Config(ConfigArgs),
    /// Initialize a CV workspace with its own cvmd.toml
    Init {
        path: PathBuf,
        /// Import an existing Markdown CV into assets/cv.md
        #[arg(long, value_name = "FILE")]
        source: Option<PathBuf>,
    },
    /// Copy the source CV into a new dated application directory
    New {
        company: String,
        position: String,
        /// Copy an existing job description into job.md
        #[arg(long, value_name = "FILE")]
        job: Option<PathBuf>,
    },
    /// Show an application and its files by company or position alias
    Show {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
    },
    /// Open a generated PDF by company or position alias
    Open {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
    },
    /// Open an application CV by company or position alias
    #[command(after_help = "Examples:\n  cvmd edit google\n  cvmd edit software_engineer")]
    Edit {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
    },
    /// Render an application CV by company or position alias
    #[command(
        after_help = "Examples:\n  cvmd build google\n  cvmd build software_engineer\n  cvmd build --verbose google"
    )]
    Build {
        /// Exact company or position alias; omit to show help
        alias: Option<String>,
        /// Show the active config, Markdown, renderer config and PDF paths on stderr
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Args)]
pub struct ConfigArgs {
    /// Use only the nearest workspace cvmd.toml; cannot combine with --config
    #[arg(long)]
    pub local: bool,
    /// Read a stored value (also the default when only KEY is given)
    #[arg(long, requires = "key", conflicts_with_all = ["list", "unset", "value"])]
    pub get: bool,
    /// List stored settings as key=value
    #[arg(long, conflicts_with_all = ["key", "value", "get", "unset"])]
    pub list: bool,
    /// Remove a setting, restoring its default
    #[arg(long, requires = "key", conflicts_with_all = ["list", "get", "value"])]
    pub unset: bool,
    /// Setting name, such as source or pdf.config_file
    pub key: Option<String>,
    /// New string value; quote paths or commands containing spaces
    #[arg(requires = "key")]
    pub value: Option<String>,
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

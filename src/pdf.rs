use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn build(config: &Config, application: &Path, verbose: bool) -> Result<PathBuf> {
    let markdown = application.join("cv.md");
    let stylesheet = config.resolve(&config.pdf.stylesheet)?;
    let resolved_stylesheet = stylesheet.canonicalize();
    if verbose {
        eprintln!("Markdown: {}", markdown.display());
        eprintln!(
            "Stylesheet: {}",
            resolved_stylesheet
                .as_deref()
                .unwrap_or(&stylesheet)
                .display()
        );
    }
    let stylesheet = resolved_stylesheet
        .with_context(|| format!("Cannot find shared stylesheet {}", stylesheet.display()))?;
    ensure!(stylesheet.is_file(), "Shared stylesheet is not a file");
    let source = File::open(&markdown).context("Cannot read application CV")?;
    let mut temporary = tempfile::NamedTempFile::new_in(application)?;
    // md-to-pdf's stdin/stdout mode lets us publish only successful renders.
    // File-backed streams also avoid pipe deadlocks with large CVs/PDFs.
    let status = Command::new(&config.pdf.renderer)
        .arg("--stylesheet")
        .arg(&stylesheet)
        .arg("--basedir")
        .arg(application)
        .current_dir(application)
        .stdin(Stdio::from(source))
        .stdout(Stdio::from(temporary.reopen()?))
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| {
            format!(
                "Cannot start PDF renderer {}; install md-to-pdf or configure pdf.renderer",
                config.pdf.renderer
            )
        })?;
    ensure!(
        status.success(),
        "PDF renderer exited unsuccessfully: {status}"
    );
    let mut signature = [0_u8; 5];
    temporary
        .read_exact(&mut signature)
        .context("Renderer did not produce a PDF on stdout")?;
    ensure!(&signature == b"%PDF-", "Renderer output is not a PDF");
    let destination = application.join("cv.pdf");
    temporary
        .persist(&destination)
        .context("Cannot save rendered PDF")?;
    Ok(destination)
}

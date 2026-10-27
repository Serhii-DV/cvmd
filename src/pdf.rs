use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn build(config: &Config, application: &Path, verbose: bool) -> Result<PathBuf> {
    let markdown = application.join("cv.md");
    let destination = config.pdf.output_path(&markdown)?;
    let input = config.resolve(&config.pdf.config_file)?;
    let resolved_input = input.canonicalize();
    if verbose {
        eprintln!("Markdown: {}", markdown.display());
        eprintln!(
            "Renderer config: {}",
            resolved_input.as_deref().unwrap_or(&input).display()
        );
        eprintln!("PDF: {}", destination.display());
    }
    let input = resolved_input
        .with_context(|| format!("Cannot find renderer config {}", input.display()))?;
    ensure!(
        input.is_file(),
        "Renderer config is not a file: {}",
        input.display()
    );
    let source = File::open(&markdown).context("Cannot read application CV")?;
    let output_directory = destination.parent().context("PDF output has no parent")?;
    fs::create_dir_all(output_directory).with_context(|| {
        format!(
            "Cannot create PDF output directory {}",
            output_directory.display()
        )
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(output_directory)?;
    // md-to-pdf's stdin/stdout mode lets us publish only successful renders.
    // File-backed streams also avoid pipe deadlocks with large CVs/PDFs.
    let status = Command::new(&config.pdf.renderer)
        .arg("--config-file")
        .arg(&input)
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
    temporary
        .persist(&destination)
        .context("Cannot save rendered PDF")?;
    Ok(destination)
}

pub fn open(config: &Config, application: &Path, alias: &str) -> Result<()> {
    let destination = config.pdf.output_path(&application.join("cv.md"))?;
    ensure!(
        destination.is_file(),
        "PDF not found: {}. Run cvmd build {:?} first",
        destination.display(),
        alias
    );
    let (browser, _) = config.effective_browser();
    let candidates: Vec<Vec<String>> = if let Some(browser) = browser {
        let arguments =
            shell_words::split(&browser).context("Invalid quoting in browser command")?;
        ensure!(!arguments.is_empty(), "BROWSER cannot be empty");
        vec![arguments]
    } else {
        #[cfg(target_os = "windows")]
        let names = ["msedge", "chrome", "firefox"];
        #[cfg(target_os = "macos")]
        let names = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Firefox.app/Contents/MacOS/firefox",
            "/Applications/Safari.app/Contents/MacOS/Safari",
        ];
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let names = ["firefox", "google-chrome", "chromium", "chromium-browser"];
        names
            .into_iter()
            .map(|name| vec![name.to_owned()])
            .collect()
    };
    for arguments in candidates {
        let result = Command::new(&arguments[0])
            .args(&arguments[1..])
            .arg(&destination)
            .spawn();
        match result {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Cannot launch browser {}", arguments[0]));
            }
        }
    }
    anyhow::bail!(
        "Cannot find a browser; install one or set browser in configuration or BROWSER to a browser command"
    )
}

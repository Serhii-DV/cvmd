use crate::{application, config::Config};
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

fn directories(parent: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("Cannot search {}", parent.display()));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        // Do not follow symlinks into unrelated directories.
        if entry.file_type()?.is_dir() {
            paths.push(entry.path());
        }
    }
    Ok(paths)
}

fn find(config: &Config, query: &str) -> Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    for year in directories(&config.root.join("applications"))? {
        for month in directories(&year)? {
            for path in directories(&month)? {
                if application::directory_date(&path).is_none() {
                    continue;
                }
                let Some(aliases) = application::aliases(&path) else {
                    continue;
                };
                if aliases.company.eq_ignore_ascii_case(query)
                    || aliases.position.eq_ignore_ascii_case(query)
                {
                    matches.push(path);
                }
            }
        }
    }
    // Fixed-width timestamps sort chronologically; show the newest first.
    matches.sort_unstable_by(|left, right| right.cmp(left));
    Ok(matches)
}

fn choose(matches: &[PathBuf]) -> Result<PathBuf> {
    eprintln!("Found {} applications (newest first):", matches.len());
    for (index, path) in matches.iter().enumerate() {
        eprintln!("  {}. {}", index + 1, path.display());
    }
    loop {
        eprint!("Select application (1-{}, or q to cancel): ", matches.len());
        io::stderr().flush()?;
        let mut input = String::new();
        ensure!(
            io::stdin().read_line(&mut input)? != 0,
            "No selection received; rerun and enter an application number"
        );
        let input = input.trim();
        ensure!(
            !input.is_empty() && !input.eq_ignore_ascii_case("q"),
            "Application selection cancelled"
        );
        if let Ok(number) = input.parse::<usize>()
            && let Some(path) = number.checked_sub(1).and_then(|index| matches.get(index))
        {
            return Ok(path.clone());
        }
        eprintln!(
            "Enter a number from 1 to {}, or q to cancel.",
            matches.len()
        );
    }
}

pub fn resolve(config: &Config, query: &str) -> Result<PathBuf> {
    let query = query.trim();
    ensure!(!query.is_empty(), "Application alias cannot be empty");
    let matches = find(config, query)?;
    let path = match matches.as_slice() {
        [] => bail!(
            "No applications found for alias {query:?}; use an exact company or position alias"
        ),
        [path] => path.clone(),
        _ => choose(&matches)?,
    };
    application::require_cv(
        path.canonicalize()
            .context("Cannot resolve selected application")?,
    )
}

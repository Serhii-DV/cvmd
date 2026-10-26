use crate::{cli::ConfigArgs, config::Config};
use anyhow::{Context, Result, bail, ensure};
use std::{fs, io::Write, path::Path};
use toml_edit::{DocumentMut, Item, Table, Value};

const KEYS: &[&str] = &[
    "root",
    "source",
    "editor",
    "pdf.renderer",
    "pdf.stylesheet",
    "pdf.filename",
];

fn setting<'a>(document: &'a DocumentMut, key: &str) -> Option<&'a Item> {
    match key.split_once('.') {
        Some((table, key)) => document.get(table)?.get(key),
        None => document.get(key),
    }
}

fn read_setting<'a>(document: &'a DocumentMut, key: &str) -> Result<&'a str> {
    setting(document, key)
        .with_context(|| format!("Setting {key} is not set"))?
        .as_str()
        .with_context(|| format!("Setting {key} must be a string"))
}

fn set(document: &mut DocumentMut, key: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "Setting {key} cannot be empty");
    if key == "editor" {
        let arguments = shell_words::split(value).context("Invalid quoting in editor command")?;
        ensure!(
            arguments.first().is_some_and(|program| !program.is_empty()),
            "Editor command cannot be empty"
        );
    }
    let slot = match key.split_once('.') {
        Some((table, key)) => document
            .entry(table)
            .or_insert(Item::Table(Table::new()))
            .as_table_like_mut()
            .context("The pdf setting must be a table")?
            .entry(key)
            .or_insert(Item::None),
        None => document.entry(key).or_insert(Item::None),
    };
    let mut replacement = Value::from(value);
    if let Some(previous) = slot.as_value() {
        *replacement.decor_mut() = previous.decor().clone();
    }
    *slot = Item::Value(replacement);
    Ok(())
}

fn unset(document: &mut DocumentMut, key: &str) -> Result<()> {
    ensure!(
        key != "root",
        "Workspace root is required and cannot be unset"
    );
    let removed = match key.split_once('.') {
        Some((table, key)) => document
            .get_mut(table)
            .and_then(Item::as_table_like_mut)
            .and_then(|table| table.remove(key)),
        None => document.remove(key),
    };
    ensure!(removed.is_some(), "Setting {key} is not set");
    Ok(())
}

fn save(path: &Path, text: &str) -> Result<()> {
    // Follow an existing symlink so updating a linked config preserves the link.
    let destination = path
        .canonicalize()
        .with_context(|| format!("Cannot resolve configuration {}", path.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(
        destination
            .parent()
            .context("Configuration has no parent")?,
    )?;
    temporary
        .as_file()
        .set_permissions(fs::metadata(&destination)?.permissions())?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&destination)
        .with_context(|| format!("Cannot save configuration {}", path.display()))?;
    Ok(())
}

pub fn run(path: &Path, args: &ConfigArgs) -> Result<()> {
    if let Some(key) = &args.key
        && !KEYS.contains(&key.as_str())
    {
        bail!(
            "Unknown configuration key {key:?}; supported keys: {}",
            KEYS.join(", ")
        );
    }
    let text = fs::read_to_string(path).with_context(|| {
        format!(
            "Cannot read {}; run `cvmd init <path>` first",
            path.display()
        )
    })?;
    let mut document: DocumentMut = text
        .parse()
        .with_context(|| format!("Invalid configuration in {}", path.display()))?;
    if args.list {
        for key in KEYS {
            if setting(&document, key).is_some() {
                println!("{key}={}", read_setting(&document, key)?);
            }
        }
        return Ok(());
    }
    let key = args
        .key
        .as_deref()
        .context("A configuration key is required")?;
    if args.unset {
        unset(&mut document, key)?;
    } else if let Some(value) = &args.value {
        set(&mut document, key, value)?;
    } else {
        println!("{}", read_setting(&document, key)?);
        return Ok(());
    }
    let updated = document.to_string();
    // Validate the entire candidate before publishing it. Keep relative paths
    // as entered rather than serializing the resolved runtime configuration.
    Config::parse(&updated, path)?;
    save(path, &updated)
}

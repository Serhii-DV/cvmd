use super::*;

fn discovered_command(workspace: &Workspace) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cvmd"));
    command
        .current_dir(workspace.temp.path())
        .env("XDG_CONFIG_HOME", workspace.temp.path().join("user-config"))
        .env("HOME", workspace.temp.path())
        .env_remove("EDITOR");
    command
}

fn local_workspace() -> Workspace {
    let mut workspace = Workspace::empty();
    workspace.config = workspace.root.join("cvmd.toml");
    success(
        discovered_command(&workspace)
            .arg("init")
            .arg(&workspace.root)
            .output()
            .unwrap(),
    );
    workspace
}

#[test]
fn init_creates_portable_local_config_and_allows_independent_workspaces() {
    let workspace = local_workspace();
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(&workspace.config).unwrap()).unwrap();
    assert_eq!(config["root"].as_str(), Some("."));
    assert_eq!(config["source"].as_str(), Some("assets/cv.md"));
    assert_eq!(config["pdf"]["stylesheet"].as_str(), Some("assets/cv.css"));
    assert!(workspace.root.join("assets/cv.md").is_file());
    assert!(workspace.root.join("assets/cv.css").is_file());
    assert!(!workspace.root.join("styles").exists());
    assert!(!workspace.temp.path().join("user-config").exists());
    let original = fs::read(&workspace.config).unwrap();
    fs::write(workspace.root.join("assets/cv.md"), "edited source").unwrap();
    failure(
        discovered_command(&workspace)
            .arg("init")
            .arg(&workspace.root)
            .output()
            .unwrap(),
        "Configuration already exists",
    );
    assert_eq!(fs::read(&workspace.config).unwrap(), original);
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "edited source"
    );
    let second = workspace.temp.path().join("second workspace");
    success(
        discovered_command(&workspace)
            .current_dir(&workspace.root)
            .arg("init")
            .arg(&second)
            .output()
            .unwrap(),
    );
    assert!(second.join("cvmd.toml").is_file());
    let moved = workspace.temp.path().join("moved workspace");
    fs::rename(&workspace.root, &moved).unwrap();
    let application = PathBuf::from(success(
        discovered_command(&workspace)
            .current_dir(&moved)
            .args(["new", "ACME", "Developer"])
            .output()
            .unwrap(),
    ));
    assert!(application.starts_with(&moved));
    assert_eq!(
        fs::read_to_string(application.join("cv.md")).unwrap(),
        "edited source"
    );
}

#[test]
fn init_source_import_refuses_to_overwrite_existing_cv_before_creating_other_files() {
    let workspace = Workspace::empty();
    fs::create_dir_all(workspace.root.join("assets")).unwrap();
    fs::write(workspace.root.join("assets/cv.md"), "edited CV").unwrap();
    failure(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .arg("--source")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cv.md"))
            .output()
            .unwrap(),
        "Source CV already exists",
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "edited CV"
    );
    assert!(!workspace.root.join("assets/cv.css").exists());
    assert!(!workspace.root.join("applications").exists());
    assert!(!workspace.config.exists());
}

#[test]
fn init_source_import_creates_assets_and_keeps_imported_cv_independent() {
    let workspace = Workspace::empty();
    let imported = workspace.temp.path().join("imported CV.md");
    fs::write(&imported, "imported CV").unwrap();
    success(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .arg("--source")
            .arg(&imported)
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "imported CV"
    );
    fs::write(&imported, "later edit").unwrap();
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "imported CV"
    );
    assert!(!workspace.root.join("styles").exists());
}

#[test]
fn local_config_is_discovered_from_nested_applications_and_explicit_config_wins() {
    let workspace = local_workspace();
    let application = workspace.application("ACME", "Developer");
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    let report = success(
        discovered_command(&workspace)
            .current_dir(&nested)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains(workspace.config.to_str().unwrap()));
    assert!(report.contains(&format!("{:<15} workspace", "Config source:")));
    assert!(report.contains(application.to_str().unwrap()));
    let alternate = workspace.temp.path().join("alternate.toml");
    fs::write(&alternate, "root = '.'\nsource = 'alternate.md'\n").unwrap();
    let value = success(
        discovered_command(&workspace)
            .current_dir(&nested)
            .arg("--config")
            .arg(&alternate)
            .args(["config", "--get", "source"])
            .output()
            .unwrap(),
    );
    assert_eq!(value, "alternate.md");
    failure(
        discovered_command(&workspace)
            .current_dir(&nested)
            .args(["--config", "missing.toml", "config", "source"])
            .output()
            .unwrap(),
        "Cannot read",
    );
}

#[test]
fn config_local_reads_updates_and_unsets_workspace_settings_from_nested_directories() {
    let workspace = local_workspace();
    let application = workspace.application("ACME", "Developer");
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    for directory in [&workspace.root, &nested] {
        success(
            discovered_command(&workspace)
                .current_dir(directory)
                .args(["config", "--local", "source", "chosen CV.md"])
                .output()
                .unwrap(),
        );
        for args in [
            vec!["config", "--local", "source"],
            vec!["config", "--local", "--get", "source"],
        ] {
            assert_eq!(
                success(
                    discovered_command(&workspace)
                        .current_dir(directory)
                        .args(args)
                        .output()
                        .unwrap()
                ),
                "chosen CV.md"
            );
        }
        let list = success(
            discovered_command(&workspace)
                .current_dir(directory)
                .args(["config", "--local", "--list"])
                .output()
                .unwrap(),
        );
        assert!(list.contains("source=chosen CV.md"));
        success(
            discovered_command(&workspace)
                .current_dir(directory)
                .args(["config", "--local", "--unset", "source"])
                .output()
                .unwrap(),
        );
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(&workspace.config).unwrap()).unwrap();
        assert!(config.get("source").is_none());
    }
}

#[test]
fn config_local_cannot_be_combined_with_an_explicit_config_path() {
    let workspace = Workspace::new();
    let original = fs::read(&workspace.config).unwrap();
    failure(
        workspace
            .command()
            .args(["config", "--local", "source", "changed.md"])
            .output()
            .unwrap(),
        "cannot be used",
    );
    failure(
        discovered_command(&workspace)
            .args(["config", "--local", "--config"])
            .arg(&workspace.config)
            .args(["source", "changed.md"])
            .output()
            .unwrap(),
        "cannot be used",
    );
    assert_eq!(fs::read(&workspace.config).unwrap(), original);
}

#[cfg(target_os = "linux")]
#[test]
fn config_local_never_falls_back_to_user_config_or_skips_invalid_workspace_config() {
    let workspace = local_workspace();
    let user_config = workspace.temp.path().join("user-config/cvmd/config.toml");
    fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    let original = "root = '.'\nsource = 'user.md'\n";
    fs::write(&user_config, original).unwrap();
    success(
        discovered_command(&workspace)
            .current_dir(&workspace.root)
            .args(["config", "--local", "source", "local.md"])
            .output()
            .unwrap(),
    );
    assert!(
        fs::read_to_string(&workspace.config)
            .unwrap()
            .contains("local.md")
    );
    assert_eq!(fs::read_to_string(&user_config).unwrap(), original);

    for args in [
        vec!["config", "--local", "source", "changed.md"],
        vec!["config", "--local", "--get", "source"],
        vec!["config", "--local", "--list"],
        vec!["config", "--local", "--unset", "source"],
    ] {
        failure(
            discovered_command(&workspace).args(&args).output().unwrap(),
            "No workspace configuration found",
        );
        assert_eq!(fs::read_to_string(&user_config).unwrap(), original);
        assert!(!workspace.temp.path().join("cvmd.toml").exists());
    }
    fs::write(&workspace.config, "root = [").unwrap();
    failure(
        discovered_command(&workspace)
            .current_dir(&workspace.root)
            .args(["config", "--local", "source", "changed.md"])
            .output()
            .unwrap(),
        "Invalid configuration",
    );
    assert_eq!(fs::read_to_string(&workspace.config).unwrap(), "root = [");
    assert_eq!(fs::read_to_string(&user_config).unwrap(), original);
}

#[test]
fn nearest_local_config_wins_and_invalid_local_config_is_not_skipped() {
    let workspace = local_workspace();
    let nested = workspace.root.join("nested");
    fs::create_dir(&nested).unwrap();
    let config = nested.join("cvmd.toml");
    fs::write(&config, "root = '.'\nsource = 'nearest.md'\n").unwrap();
    assert_eq!(
        success(
            discovered_command(&workspace)
                .current_dir(&nested)
                .args(["config", "source"])
                .output()
                .unwrap()
        ),
        "nearest.md"
    );
    success(
        discovered_command(&workspace)
            .current_dir(&nested)
            .args(["config", "source", "changed.md"])
            .output()
            .unwrap(),
    );
    assert!(fs::read_to_string(&config).unwrap().contains("changed.md"));
    assert!(
        !fs::read_to_string(&workspace.config)
            .unwrap()
            .contains("changed.md")
    );
    fs::write(&config, "root = [").unwrap();
    failure(
        discovered_command(&workspace)
            .current_dir(&nested)
            .arg("info")
            .output()
            .unwrap(),
        "Invalid configuration",
    );
}

#[test]
fn config_source_uses_external_relative_absolute_and_home_paths_for_future_applications() {
    let workspace = local_workspace();
    let existing = workspace.application("Existing", "Developer");
    fs::write(existing.join("cv.md"), "tailored CV").unwrap();
    let sources = workspace.root.join("sources");
    fs::create_dir(&sources).unwrap();
    let relative = sources.join("CV with spaces.md");
    let external = workspace.temp.path().join("external CV.md");
    fs::write(&relative, "relative source").unwrap();
    fs::write(&external, "external source").unwrap();
    let original_source = fs::read(workspace.root.join("assets/cv.md")).unwrap();
    for (value, expected) in [
        ("sources/CV with spaces.md", "relative source"),
        (external.to_str().unwrap(), "external source"),
        ("~/external CV.md", "external source"),
    ] {
        success(
            discovered_command(&workspace)
                .current_dir(&existing)
                .args(["config", "source", value])
                .output()
                .unwrap(),
        );
        assert_eq!(
            success(
                workspace
                    .command()
                    .args(["config", "source"])
                    .output()
                    .unwrap()
            ),
            value
        );
        let application = PathBuf::from(success(
            discovered_command(&workspace)
                .current_dir(&sources)
                .args(["new", "New", "Developer"])
                .output()
                .unwrap(),
        ));
        assert_eq!(
            fs::read_to_string(application.join("cv.md")).unwrap(),
            expected
        );
    }
    assert_eq!(
        fs::read(workspace.root.join("assets/cv.md")).unwrap(),
        original_source
    );
    assert_eq!(
        fs::read_to_string(existing.join("cv.md")).unwrap(),
        "tailored CV"
    );
    assert_eq!(fs::read_to_string(external).unwrap(), "external source");
}

#[test]
fn config_edits_preserve_comments_unknown_fields_relative_root_and_other_settings() {
    let workspace = Workspace::new();
    let original = "# Personal settings\nroot = '../CV workspace with spaces' # relative\nsource = 'assets/cv.md' # source\ncustom = 'keep me'\n\n[pdf]\nrenderer = 'md-to-pdf' # tool\nstylesheet = 'assets/cv.css'\n";
    fs::write(&workspace.config, original).unwrap();
    success(
        workspace
            .command()
            .args(["config", "source", "CV with spaces.md"])
            .output()
            .unwrap(),
    );
    success(
        workspace
            .command()
            .args(["config", "pdf.renderer", "custom-renderer"])
            .output()
            .unwrap(),
    );
    let updated = fs::read_to_string(&workspace.config).unwrap();
    assert!(updated.contains("# Personal settings"));
    assert!(updated.contains("root = '../CV workspace with spaces' # relative"));
    assert!(updated.contains("source = \"CV with spaces.md\" # source"));
    assert!(updated.contains("custom = 'keep me'"));
    assert!(updated.contains("renderer = \"custom-renderer\" # tool"));
    assert!(updated.contains("stylesheet = 'assets/cv.css'"));
    let listed = success(
        workspace
            .command()
            .args(["config", "--list"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        listed,
        "root=../CV workspace with spaces\nsource=CV with spaces.md\npdf.renderer=custom-renderer\npdf.stylesheet=assets/cv.css"
    );
    assert_eq!(fs::read_to_string(&workspace.config).unwrap(), updated);
}

#[test]
fn config_unset_restores_defaults_and_nested_settings_can_be_added() {
    let workspace = Workspace::new();
    for (key, value) in [
        ("editor", "code --wait"),
        ("source", "other.md"),
        ("pdf.renderer", "other-renderer"),
        ("pdf.stylesheet", "other.css"),
        ("pdf.filename", "My CV.pdf"),
    ] {
        success(
            workspace
                .command()
                .args(["config", key, value])
                .output()
                .unwrap(),
        );
        assert_eq!(
            success(
                workspace
                    .command()
                    .args(["config", "--get", key])
                    .output()
                    .unwrap()
            ),
            value
        );
        success(
            workspace
                .command()
                .args(["config", "--unset", key])
                .output()
                .unwrap(),
        );
        failure(
            workspace.command().args(["config", key]).output().unwrap(),
            "is not set",
        );
    }
    assert!(
        workspace
            .application("ACME", "Developer")
            .join("cv.md")
            .is_file()
    );
    let report = success(workspace.command().arg("info").output().unwrap());
    assert!(report.contains("micro [default]"));
    assert!(report.contains("md-to-pdf"));
    fs::write(
        &workspace.config,
        format!("root = {:?}\n", workspace.root.to_str().unwrap()),
    )
    .unwrap();
    success(
        workspace
            .command()
            .args(["config", "pdf.stylesheet", "assets/cv.css"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(
            workspace
                .command()
                .args(["config", "pdf.stylesheet"])
                .output()
                .unwrap()
        ),
        "assets/cv.css"
    );
    success(
        workspace
            .command()
            .args(["config", "root", "../CV workspace with spaces"])
            .output()
            .unwrap(),
    );
    assert!(workspace.application("Other", "Developer").is_dir());
}

#[test]
fn invalid_config_commands_leave_existing_configuration_unchanged() {
    let workspace = Workspace::new();
    let original = fs::read(&workspace.config).unwrap();
    for (args, error) in [
        (
            vec!["config", "unknown", "value"],
            "Unknown configuration key",
        ),
        (vec!["config", "pdf.unknown"], "Unknown configuration key"),
        (vec!["config", "source", ""], "cannot be empty"),
        (vec!["config", "root", "  "], "cannot be empty"),
        (vec!["config", "editor", "'"], "Invalid quoting"),
        (vec!["config", "--unset", "root"], "cannot be unset"),
        (vec!["config", "--get"], "required"),
        (vec!["config", "--list", "source"], "cannot be used"),
        (
            vec!["config", "--unset", "source", "other.md"],
            "cannot be used",
        ),
        (
            vec!["config", "--get", "source", "other.md"],
            "cannot be used",
        ),
    ] {
        failure(workspace.command().args(args).output().unwrap(), error);
        assert_eq!(fs::read(&workspace.config).unwrap(), original);
    }
    fs::write(&workspace.config, "root = [").unwrap();
    failure(
        workspace
            .command()
            .args(["config", "source", "new.md"])
            .output()
            .unwrap(),
        "Invalid configuration",
    );
    assert_eq!(fs::read_to_string(&workspace.config).unwrap(), "root = [");
    fs::write(&workspace.config, "root = 42\nsource = 'original.md'\n").unwrap();
    failure(
        workspace
            .command()
            .args(["config", "source", "new.md"])
            .output()
            .unwrap(),
        "Invalid configuration",
    );
    assert_eq!(
        fs::read_to_string(&workspace.config).unwrap(),
        "root = 42\nsource = 'original.md'\n"
    );
}

#[test]
fn bare_config_shows_help_without_loading_or_creating_configuration() {
    let workspace = Workspace::empty();
    let expected = success(
        workspace
            .command()
            .args(["config", "--help"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(workspace.command().arg("config").output().unwrap()),
        expected
    );
    failure(
        workspace
            .command()
            .args(["config", "source", "new.md"])
            .output()
            .unwrap(),
        "Cannot read",
    );
    assert!(!workspace.config.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn local_config_takes_priority_over_legacy_config_without_modifying_it() {
    let workspace = local_workspace();
    let legacy = workspace.temp.path().join("user-config/cvmd/config.toml");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let original = "root = '.'\nsource = 'legacy.md'\n";
    fs::write(&legacy, original).unwrap();
    assert_eq!(
        success(
            discovered_command(&workspace)
                .current_dir(&workspace.root)
                .args(["config", "source"])
                .output()
                .unwrap()
        ),
        "assets/cv.md"
    );
    assert_eq!(
        success(
            discovered_command(&workspace)
                .args(["config", "source"])
                .output()
                .unwrap()
        ),
        "legacy.md"
    );
    success(
        discovered_command(&workspace)
            .current_dir(&workspace.root)
            .args(["config", "source", "local.md"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&legacy).unwrap(), original);
    fs::write(&workspace.config, "root = [").unwrap();
    failure(
        discovered_command(&workspace)
            .current_dir(&workspace.root)
            .arg("info")
            .output()
            .unwrap(),
        "Invalid configuration",
    );
    assert_eq!(fs::read_to_string(&legacy).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn config_edits_preserve_symlinks_and_file_permissions() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let workspace = Workspace::new();
    let target = workspace.temp.path().join("linked-target.toml");
    fs::rename(&workspace.config, &target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, &workspace.config).unwrap();
    success(
        workspace
            .command()
            .args(["config", "source", "external.md"])
            .output()
            .unwrap(),
    );
    assert!(
        fs::symlink_metadata(&workspace.config)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(fs::read_to_string(&target).unwrap().contains("external.md"));
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[cfg(unix)]
#[test]
fn verbose_build_reports_local_config_and_the_actual_markdown_and_external_stylesheet() {
    let workspace = local_workspace();
    let source = workspace.temp.path().join("external CV.md");
    let stylesheet = workspace.temp.path().join("external style.css");
    fs::write(&source, "external source CV").unwrap();
    fs::write(&stylesheet, "body { color: navy; }").unwrap();
    for (key, value) in [
        ("source", source.to_str().unwrap()),
        ("pdf.stylesheet", stylesheet.to_str().unwrap()),
    ] {
        success(
            discovered_command(&workspace)
                .current_dir(&workspace.root)
                .args(["config", "--local", key, value])
                .output()
                .unwrap(),
        );
    }
    let application = workspace.application("ACME", "Developer");
    assert_eq!(
        fs::read_to_string(application.join("cv.md")).unwrap(),
        "external source CV"
    );
    fs::write(application.join("cv.md"), "tailored application CV").unwrap();
    // Future source changes must not replace this application's tailored CV.
    fs::write(&source, "source changed after new").unwrap();
    fs::remove_file(workspace.root.join("assets/cv.md")).unwrap();
    fs::remove_file(workspace.root.join("assets/cv.css")).unwrap();
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    let renderer = unix::executable(
        &workspace,
        "verbose-renderer",
        "printf '%s\\n' \"$@\" > \"$ARGUMENTS\"\ncat > \"$INPUT\"\nprintf '%s' '%PDF-1.7 verbose'",
    );
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    let arguments = workspace.temp.path().join("render-arguments");
    let input = workspace.temp.path().join("render-input");
    for flag in ["--verbose", "-v"] {
        let output = discovered_command(&workspace)
            .current_dir(&nested)
            .args(["build", flag, "acme"])
            .env("ARGUMENTS", &arguments)
            .env("INPUT", &input)
            .output()
            .unwrap();
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            diagnostics,
            format!(
                "Config: {} [workspace]\nMarkdown: {}\nStylesheet: {}\nPDF: {}\n",
                workspace.config.display(),
                application.join("cv.md").display(),
                stylesheet.canonicalize().unwrap().display(),
                application.join("cv.pdf").display()
            )
        );
        assert_eq!(
            success(output),
            format!("Built {}", application.join("cv.pdf").display())
        );
        assert_eq!(
            fs::read_to_string(&input).unwrap(),
            "tailored application CV"
        );
        assert_eq!(
            fs::read_to_string(&arguments).unwrap(),
            format!(
                "--stylesheet\n{}\n--basedir\n{}\n",
                stylesheet.canonicalize().unwrap().display(),
                application.display()
            )
        );
    }
    let normal = discovered_command(&workspace)
        .current_dir(&nested)
        .args(["build", "acme"])
        .env("ARGUMENTS", &arguments)
        .env("INPUT", &input)
        .output()
        .unwrap();
    assert!(normal.stderr.is_empty());
    assert_eq!(
        success(normal),
        format!("Built {}", application.join("cv.pdf").display())
    );
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "source changed after new"
    );
    assert_eq!(
        fs::read_to_string(application.join("cv.md")).unwrap(),
        "tailored application CV"
    );
}

#[test]
fn verbose_build_reports_attempted_paths_when_stylesheet_or_config_is_missing() {
    let workspace = local_workspace();
    let application = workspace.application("ACME", "Developer");
    let pdf = application.join("cv.pdf");
    fs::write(&pdf, "%PDF-1.7 previous").unwrap();
    workspace.configure(
        "pdf",
        "stylesheet",
        workspace
            .temp
            .path()
            .join("missing style.css")
            .to_str()
            .unwrap(),
    );
    let output = discovered_command(&workspace)
        .current_dir(&application)
        .args(["build", "--verbose", "acme"])
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    for expected in [
        format!("Config: {} [workspace]", workspace.config.display()),
        format!("Markdown: {}", application.join("cv.md").display()),
        format!(
            "Stylesheet: {}",
            workspace.temp.path().join("missing style.css").display()
        ),
    ] {
        assert!(diagnostics.contains(&expected), "{diagnostics}");
    }
    assert!(output.stdout.is_empty());
    failure(output, "Cannot find shared stylesheet");
    assert_eq!(fs::read_to_string(&pdf).unwrap(), "%PDF-1.7 previous");
    fs::remove_file(&workspace.config).unwrap();
    let output = workspace
        .command()
        .args(["build", "-v", "acme"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains(&format!(
        "Config: {} [--config]",
        workspace.config.display()
    )));
    failure(output, "Cannot read");
    assert_eq!(fs::read_to_string(&pdf).unwrap(), "%PDF-1.7 previous");
}

#[cfg(unix)]
#[test]
fn configured_tools_edit_and_build_with_discovered_workspace_config() {
    let workspace = local_workspace();
    let application = workspace.application("ACME", "Developer");
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    let renderer = unix::executable(
        &workspace,
        "local-renderer",
        "cat >/dev/null\nprintf '%s' '%PDF-1.7 local'",
    );
    let editor = unix::executable(
        &workspace,
        "local-editor",
        "printf '%s' \"$1\" > \"$CAPTURE\"",
    );
    let capture = workspace.temp.path().join("edited-cv");
    for (key, value) in [
        ("pdf.renderer", renderer.to_str().unwrap()),
        ("editor", editor.to_str().unwrap()),
    ] {
        success(
            discovered_command(&workspace)
                .current_dir(&nested)
                .args(["config", key, value])
                .output()
                .unwrap(),
        );
    }
    for command in ["edit", "build"] {
        success(
            discovered_command(&workspace)
                .current_dir(&nested)
                .env("CAPTURE", &capture)
                .args([command, "acme"])
                .output()
                .unwrap(),
        );
    }
    assert_eq!(
        PathBuf::from(fs::read_to_string(capture).unwrap())
            .canonicalize()
            .unwrap(),
        application.join("cv.md").canonicalize().unwrap()
    );
    assert_eq!(
        fs::read(application.join("cv.pdf")).unwrap(),
        b"%PDF-1.7 local"
    );
}

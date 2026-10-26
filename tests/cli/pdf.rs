use super::*;

#[cfg(unix)]
#[test]
fn renderer_config_paths_resolve_from_the_workspace_without_overriding_their_stylesheets() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    let renderer = unix::executable(
        &workspace,
        "config-renderer",
        "printf '%s\\n' \"$@\" > \"$ARGUMENTS\"\npwd > \"$WORKING_DIRECTORY\"\ncat > \"$INPUT\"\nprintf '%s' '%PDF-1.7 configured'",
    );
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    // CSS loading is handled by the JavaScript config inside the renderer.
    fs::remove_file(workspace.root.join("assets/cv.css")).unwrap();
    let custom = workspace.root.join("assets/custom config.js");
    fs::write(&custom, "module.exports = {};").unwrap();
    let external = workspace.temp.path().join("external config.js");
    fs::write(&external, "module.exports = {};").unwrap();
    let arguments = workspace.temp.path().join("renderer-arguments");
    let working_directory = workspace.temp.path().join("renderer-directory");
    let input = workspace.temp.path().join("renderer-input");

    for (setting, expected) in [
        (
            "assets/pdf.config.js",
            workspace.root.join("assets/pdf.config.js"),
        ),
        ("assets/custom config.js", custom),
        (external.to_str().unwrap(), external.clone()),
        ("~/external config.js", external.clone()),
    ] {
        success(
            workspace
                .command()
                .args(["config", "pdf.config_file", setting])
                .output()
                .unwrap(),
        );
        assert_eq!(
            success(
                workspace
                    .command()
                    .args(["config", "--get", "pdf.config_file"])
                    .output()
                    .unwrap()
            ),
            setting
        );
        let report = success(workspace.command().arg("info").output().unwrap());
        assert!(report.contains("Renderer config:"));
        assert!(!report.contains("Stylesheet:"));
        let output = workspace
            .command()
            .current_dir(&nested)
            .env("HOME", workspace.temp.path())
            .env("ARGUMENTS", &arguments)
            .env("WORKING_DIRECTORY", &working_directory)
            .env("INPUT", &input)
            .args(["build", "--verbose", "acme"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!(
            "Renderer config: {}",
            expected.canonicalize().unwrap().display()
        )));
        success(output);
        assert_eq!(
            fs::read_to_string(&arguments).unwrap(),
            format!(
                "--config-file\n{}\n--basedir\n{}\n",
                expected.canonicalize().unwrap().display(),
                application.display()
            )
        );
        assert_eq!(
            fs::read_to_string(&working_directory).unwrap().trim(),
            application.to_str().unwrap()
        );
        assert_eq!(
            fs::read(&input).unwrap(),
            fs::read(application.join("cv.md")).unwrap()
        );
        assert_eq!(
            fs::read(application.join("output/cv.pdf")).unwrap(),
            b"%PDF-1.7 configured"
        );
    }
}

#[cfg(unix)]
#[test]
fn missing_or_nonfile_renderer_config_preserves_the_pdf_without_starting_the_renderer() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    let pdf = application.join("output/cv.pdf");
    fs::write(&pdf, "%PDF-1.7 previous").unwrap();
    let marker = workspace.temp.path().join("renderer-started");
    let renderer = unix::executable(&workspace, "unused-renderer", "touch \"$MARKER\"");
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    for (setting, message) in [
        ("assets/missing config.js", "Cannot find renderer config"),
        ("assets", "Renderer config is not a file"),
    ] {
        workspace.configure("pdf", "config_file", setting);
        let output = workspace
            .command()
            .env("MARKER", &marker)
            .args(["build", "--verbose", "acme"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!(
            "Renderer config: {}",
            workspace.root.join(setting).display()
        )));
        failure(output, message);
        assert!(!marker.exists());
        assert_eq!(fs::read_to_string(&pdf).unwrap(), "%PDF-1.7 previous");
        assert_eq!(fs::read_dir(application.join("output")).unwrap().count(), 1);
    }
    workspace.configure("pdf", "config_file", "");
    failure(
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "pdf.config_file must be a nonempty path",
    );
}

#[cfg(unix)]
#[test]
fn configured_pdf_filename_is_used_and_unsetting_restores_the_markdown_name() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    let renderer = unix::executable(
        &workspace,
        "filename-renderer",
        "cat >/dev/null\nprintf '%s' '%PDF-1.7 named'",
    );
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    let markdown = fs::read(application.join("cv.md")).unwrap();
    let identity = fs::read(application.join("application.toml")).unwrap();
    let default_pdf = application.join("output/cv.pdf");
    fs::write(&default_pdf, "%PDF-1.7 existing default").unwrap();
    let previous_output = application.join("cv.pdf");
    fs::write(&previous_output, "%PDF-1.7 previous location").unwrap();

    for filename in ["Jane Doe CV.pdf", "Résumé 北欧.PDF"] {
        success(
            workspace
                .command()
                .args(["config", "pdf.filename", filename])
                .output()
                .unwrap(),
        );
        assert_eq!(
            success(
                workspace
                    .command()
                    .args(["config", "--get", "pdf.filename"])
                    .output()
                    .unwrap()
            ),
            filename
        );
        assert!(
            success(
                workspace
                    .command()
                    .args(["config", "--list"])
                    .output()
                    .unwrap()
            )
            .contains(&format!("pdf.filename={filename}"))
        );
        let destination = application.join("output").join(filename);
        let output = workspace
            .command()
            .args(["build", "--verbose", "acme"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&format!("PDF: {}", destination.display()))
        );
        assert_eq!(success(output), format!("Built {}", destination.display()));
        assert_eq!(fs::read(&destination).unwrap(), b"%PDF-1.7 named");
        assert_eq!(
            fs::read_to_string(&default_pdf).unwrap(),
            "%PDF-1.7 existing default"
        );
    }

    success(
        workspace
            .command()
            .args(["config", "--unset", "pdf.filename"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap()
        ),
        format!("Built {}", default_pdf.display())
    );
    assert_eq!(fs::read(&default_pdf).unwrap(), b"%PDF-1.7 named");
    assert_eq!(
        fs::read(application.join("output/Jane Doe CV.pdf")).unwrap(),
        b"%PDF-1.7 named"
    );
    assert_eq!(fs::read(application.join("cv.md")).unwrap(), markdown);
    assert_eq!(
        fs::read_to_string(previous_output).unwrap(),
        "%PDF-1.7 previous location"
    );
    assert_eq!(
        fs::read(application.join("application.toml")).unwrap(),
        identity
    );
}

#[cfg(unix)]
#[test]
fn failed_renders_preserve_the_configured_pdf_and_leave_no_new_output() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    workspace.configure("pdf", "filename", "My CV.pdf");
    let custom_pdf = application.join("output/My CV.pdf");
    let default_pdf = application.join("output/cv.pdf");
    fs::write(&custom_pdf, "%PDF-1.7 previous custom").unwrap();
    fs::write(&default_pdf, "%PDF-1.7 previous default").unwrap();
    let original_cv = fs::read(application.join("cv.md")).unwrap();
    let initial_count = fs::read_dir(&application).unwrap().count();
    let initial_output_count = fs::read_dir(application.join("output")).unwrap().count();
    for (name, script, message) in [
        (
            "failed-named-renderer",
            "printf 'partial PDF'\nexit 7",
            "PDF renderer exited unsuccessfully",
        ),
        (
            "invalid-named-renderer",
            "printf 'not a PDF'",
            "Renderer output is not a PDF",
        ),
    ] {
        let renderer = unix::executable(&workspace, name, script);
        workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
        failure(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap(),
            message,
        );
        assert_eq!(
            fs::read_to_string(&custom_pdf).unwrap(),
            "%PDF-1.7 previous custom"
        );
        assert_eq!(
            fs::read_to_string(&default_pdf).unwrap(),
            "%PDF-1.7 previous default"
        );
        assert_eq!(fs::read(application.join("cv.md")).unwrap(), original_cv);
        assert_eq!(fs::read_dir(&application).unwrap().count(), initial_count);
        assert_eq!(
            fs::read_dir(application.join("output")).unwrap().count(),
            initial_output_count
        );
    }
    workspace.configure("pdf", "filename", "New failed CV.pdf");
    failure(
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "Renderer output is not a PDF",
    );
    assert!(!application.join("output/New failed CV.pdf").exists());
    assert_eq!(fs::read_dir(&application).unwrap().count(), initial_count);
    assert_eq!(
        fs::read_dir(application.join("output")).unwrap().count(),
        initial_output_count
    );
}

#[test]
fn blocked_output_directory_preserves_existing_files() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    let markdown = fs::read(application.join("cv.md")).unwrap();
    let blocked_directory = application.join("output");
    fs::write(&blocked_directory, "existing user file").unwrap();
    let previous_pdf = application.join("cv.pdf");
    fs::write(&previous_pdf, "%PDF-1.7 existing").unwrap();
    workspace.configure("pdf", "renderer", "/nonexistent/renderer");
    failure(
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "Cannot create PDF output directory",
    );
    assert_eq!(
        fs::read_to_string(blocked_directory).unwrap(),
        "existing user file"
    );
    assert_eq!(
        fs::read_to_string(previous_pdf).unwrap(),
        "%PDF-1.7 existing"
    );
    assert_eq!(fs::read(application.join("cv.md")).unwrap(), markdown);
}

#[test]
fn invalid_pdf_filenames_are_rejected_without_changing_config_or_existing_files() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    let original_config = fs::read(&workspace.config).unwrap();
    let original_cv = fs::read(application.join("cv.md")).unwrap();
    let pdf = application.join("output/cv.pdf");
    fs::write(&pdf, "%PDF-1.7 previous").unwrap();
    for filename in [
        "",
        " ",
        ".",
        "..",
        "../outside.pdf",
        "nested/CV.pdf",
        "nested\\CV.pdf",
        "/absolute.pdf",
        "C:\\absolute.pdf",
        "cv.md",
        "application.toml",
        "resume",
        ".pdf",
    ] {
        let output = workspace
            .command()
            .args(["config", "pdf.filename", filename])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "accepted invalid filename {filename:?}"
        );
        assert_eq!(fs::read(&workspace.config).unwrap(), original_config);
        // The same validation applies to settings edited directly in TOML.
        workspace.configure("pdf", "filename", filename);
        failure(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap(),
            "pdf.filename",
        );
        assert_eq!(fs::read(application.join("cv.md")).unwrap(), original_cv);
        assert_eq!(fs::read_to_string(&pdf).unwrap(), "%PDF-1.7 previous");
        fs::write(&workspace.config, &original_config).unwrap();
    }
}

# Repository Guidelines

## Project Structure & Module Organization

`cvmd` is a Rust 2024 CLI. Keep modules focused:

- `src/main.rs`: entry point, command dispatch, and error reporting.
- `src/cli.rs`: Clap arguments and subcommands.
- `src/config.rs`: TOML configuration and path resolution.
- `src/workspace.rs`: workspace initialization.
- `src/application.rs`: application creation, discovery, and editing.
- `src/pdf.rs`: external renderer execution and PDF publication.

`assets/` contains the starter CV and shared CSS. `tests/cli.rs` contains integration
tests; `tests/fixtures/` holds synthetic inputs. `.github/workflows/ci.yml` defines CI.

## Build, Test, and Development Commands

Use a current stable Rust toolchain:

- `cargo build --locked`: build the development binary.
- `cargo run -- --help`: inspect local CLI usage.
- `cargo run -- --config /tmp/cvmd-config.toml init /tmp/cvmd-workspace`: initialize an isolated workspace.
- `cargo test --locked`: run the test suite.
- `cargo fmt --all`: format Rust code; add `-- --check` to verify formatting.
- `cargo clippy --locked --all-targets -- -D warnings`: run CI lint checks.

Real PDF builds require separately installed `md-to-pdf` and Chromium dependencies.
Tests use stubs and do not require them.

## Coding Style & Naming Conventions

Use rustfmt defaults, including four-space indentation. Use `snake_case` for modules,
functions, and variables, and `UpperCamelCase` for types. Follow SOLID, DRY, and KISS;
prefer focused functions over speculative abstractions. Return `anyhow::Result` with
useful error context. Invoke external programs directly without shell interpolation.
Keep `Cargo.lock` updated when dependencies change.

## Testing Guidelines

Use Rust's built-in test harness and isolated `tempfile` workspaces. Name tests after
observable behavior, such as `invalid_inputs_do_not_leave_partial_applications`.
Cover changed behavior, especially path resolution, exact TOML names, collisions,
and preservation of existing files after failures. Shell-based process tests are
Unix-only. No numeric coverage threshold is configured. Run all CI checks before
submitting changes.

## Commit & Pull Request Guidelines

History contains short, plain summaries such as `First version draft`; no formal
commit convention is established. Write concise, action-oriented subjects. Keep
PRs focused, describe resulting behavior, link relevant issues, and report validation.
Update the README for CLI or configuration changes.

## Changelog

Maintain `CHANGELOG.md` at the repository root. Add compact, user-facing bullets
under `Unreleased`, grouped by `Added`, `Changed`, `Fixed`, or `Removed`. Mention
breaking changes and required migration steps. Avoid implementation logs.

## Data & Configuration Safety

Keep personal CVs outside this repository and use synthetic fixtures. Preserve
filesystem-based organization and exact company/position names in `application.toml`.
Never overwrite existing applications or edited CVs; failed renders must preserve
previous PDFs.

# cvmd

A file-first Rust CLI for managing, tailoring and exporting Markdown CVs to PDF.
Keep one source CV, create an independent copy for each application, edit it with
your preferred tools, and render it with shared styling.

## Install

Install a current stable Rust toolchain, then run from this repository:

```sh
cargo install --path . --locked
```

With Make installed, `make build` runs the same command to build and install
`cvmd` into Cargo's binary directory. Run `make` or `make help` to list targets.

PDF generation uses [md-to-pdf](https://github.com/simonhaenisch/md-to-pdf), installed
separately with Node.js and its Chromium dependencies:

```sh
npm install --global md-to-pdf
```

The other commands work without a PDF renderer. Editing defaults to `micro` unless
you set `$EDITOR` or configure another editor.

## Workflow

Keep your personal CV workspace outside this source repository, ideally in a
private, backed-up directory.

```sh
# Import your existing CV, or omit --source to use a starter template.
cvmd init ~/CVs --source ~/Documents/my-cv.md
cd ~/CVs

# Create an application and enter its directory. --job is optional.
cd "$(cvmd new 'ACME / R&D A/S' 'Senior PHP/TS Developer' --job ~/Downloads/job.md)"

cvmd edit acme_r_d_a_s
cvmd build acme_r_d_a_s
```

`new` prints the new directory's absolute path. It copies the source CV without
modifying it, copies the supplied job description (or creates an empty `job.md`),
and creates empty notes. Applications use the local date and time:

```text
~/CVs/
├── cvmd.toml
├── assets/
│   ├── cv.md
│   ├── cv.css
│   ├── columns.css
│   └── pdf.config.js
└── applications/
    └── 2026/
        └── 10/
            └── 2026-10-26_19-12-30.acme_r_d_a_s.senior_php_ts_developer/
                ├── application.toml
                ├── cv.md
                ├── job.md
                ├── notes.md
                └── output/   # created by build
                    └── cv.pdf
```

Folder names follow `YYYY-MM-DD_HH-MM-SS.company_alias.position_alias`, using local
time with second precision. Applications created in different seconds sort by
time; those created in the same second sort by alias. Aliases use lowercase ASCII
letters, digits, and underscores, with each alias limited to roughly 48 characters. For example,
`Software Engineer` becomes `software_engineer`. Names containing no ASCII letters
or digits use `unnamed`. Identical timestamp/name collisions get `-2`, `-3`, etc.;
existing applications and edited CVs are never overwritten.

Each position has an `application.toml` that preserves its exact original names,
including case, Unicode, punctuation, slashes, and quotes:

```toml
company = "ACME / R&D A/S"
position = "Senior PHP/TS Developer"
```

The filesystem remains the source of application organization: the date is
derived from the directory, and no database, statuses, or index are maintained.
The identity file is not required for `edit` or `build` to discover an application.

Pass an alias to `edit` or `build` to select an application. Without an alias, either
command prints its help and exits successfully, including inside an application
directory. No configuration is needed to display help. You can also edit all
Markdown and TOML files directly without the CLI.

### Find an application by alias

Pass a company or position alias to `show`, `edit`, or `build` to search the configured
workspace. Run from anywhere inside the workspace, or select its config explicitly
when working elsewhere:

```sh
cvmd show google
cvmd edit google
cvmd build software_engineer
cvmd --config /path/to/config.toml edit acme_r_d_a_s
```

`show` displays the application names, folder, and existing files recursively, including built PDFs. If metadata is unavailable, it displays the folder aliases instead. Symbolic links are not followed. Bare `show` displays help.

Matching is exact and case-insensitive. A single match is used directly; multiple
matches produce a numbered list, newest first. Enter a number to select a folder,
or enter `q` or a blank line to cancel. Invalid numbers prompt again; end-of-input
cancels without launching an editor or renderer. Lists and prompts go to stderr.
No match produces a helpful error. `cvmd info`, run inside an application directory,
shows its company and position aliases.

New folders can be searched without `application.toml`, since their aliases are
encoded in the folder name. Existing folders with fractional seconds remain
supported. Date-only folders remain usable without being renamed; alias search
derives their aliases from valid `application.toml` metadata.

## Configuration

Run `cvmd info` to inspect the active config file, its selection source, resolved
workspace/source/renderer config paths and existence, effective editor and its source,
and PDF renderer availability. Inside an application (including subdirectories),
it also shows the application path, searchable aliases, and original company/position names.

```sh
cvmd info
cvmd --config /path/to/config.toml info
```

This command is read-only and does not launch the editor or renderer. Missing or
invalid configuration prints the selected config path and an actionable error,
then exits unsuccessfully. Missing workspace files and unavailable renderers are
reported without failing; missing or invalid application metadata leaves the rest
of the report available.

### Workspace configuration

`init` creates `cvmd.toml` in the workspace root, beside `assets/` and
`applications/`. Settings are selected in this order:

1. The global `--config /path/to/config.toml` option, when supplied.
2. The nearest `cvmd.toml` in the current directory or its parents.
3. The existing user configuration in the OS configuration directory (legacy fallback).

Legacy config locations are:

- Linux/WSL: `$XDG_CONFIG_HOME/cvmd/config.toml`, or `~/.config/cvmd/config.toml`.
- macOS: `~/Library/Application Support/cvmd/config.toml`.
- Windows: `%APPDATA%\cvmd\config.toml`.

Only one file is used; settings from different files are not merged. An invalid
selected config produces an error rather than silently selecting another file.
New workspaces do not write to the OS configuration directory. To work outside a
workspace, supply `--config` on each invocation or use an existing legacy config.

`init` always targets the workspace being initialized, regardless of the current
directory's configuration. It refuses to replace an existing config. An explicit
`--config` with `init` writes to that path instead of creating `cvmd.toml`. When
initializing an existing workspace, it preserves the source CV, CSS, and renderer config; an explicit
`--source` import fails if `assets/cv.md` already exists.

```toml
root = "."
source = "assets/cv.md"
editor = "micro"

[pdf]
renderer = "md-to-pdf"
config_file = "assets/pdf.config.js"
# filename = "Jane Doe CV.pdf"  # optional; otherwise use the Markdown stem
```

`pdf.config_file` defaults to `assets/pdf.config.js` and selects the JavaScript
config that defines stylesheets, PDF options, and Markdown extensions.

`~` and `~/…` are expanded in configuration paths. Relative `root` paths resolve
against the configuration file's directory; relative source and renderer config paths
resolve against the workspace root. Input `--source` and `--job` paths resolve
against the command's current directory.

New local configs use `root = "."`, so moving the whole workspace keeps its
relative CV and renderer config paths working. Absolute paths to external files still
refer to those external locations.

### Read and change settings

Use Git-style key/value commands from inside your workspace:

```sh
cvmd config --list
cvmd config source                         # read the stored value
cvmd config --get source                   # explicit read
cvmd config source ~/Documents/my-cv.md     # use an external CV for new applications
cvmd config --local source ~/Documents/my-cv.md  # require the current workspace config
cvmd config editor 'code --wait'
cvmd config pdf.config_file assets/custom-pdf.config.js
cvmd config --local pdf.filename 'Jane Doe CV.pdf'
cvmd config --unset editor                 # restore the default
```

Use `--local` to explicitly read or edit the nearest `cvmd.toml` in the current
directory or its parents. This works from an application subdirectory as well as
the workspace root and applies to reads, writes, `--list`, and `--unset`. If no
workspace config exists, the command fails without changing the user config.
For a folder initialized without a local config, run `cvmd init .` from its root
first, then use `cvmd config --local KEY VALUE`. Without `--local`, the normal
selection order above applies. `--local` cannot be combined with `--config FILE`;
use `--config FILE` to target a specific config from anywhere.

Supported keys are `root`, `source`, `editor`, `pdf.renderer`, `pdf.config_file`,
and `pdf.filename`.
Values are strings; quote values containing spaces. Reads show stored values,
without expanding paths or applying `$EDITOR`; `--list` shows supported keys
present in the file as `key=value`. Reading or unsetting an absent key fails.
Bare `cvmd config` shows help. Use `cvmd info` to see resolved paths and defaults.

`--unset` restores the default for `source` (`assets/cv.md`), `editor` (`micro`),
`pdf.renderer` (`md-to-pdf`), or `pdf.config_file` (`assets/pdf.config.js`).
Unsetting `pdf.filename` restores the name derived from the Markdown file.
The required `root` setting cannot be unset.
Empty values, unknown keys, and invalid editor
quoting are rejected. Changes preserve comments and unrelated settings and are
written atomically to the selected file. Config commands require an existing file;
they do not initialize a workspace.

Setting `source` references the selected CV directly: each future `new` copies its
current contents. Existing application CVs and `assets/cv.md` remain untouched.
Relative `source` paths resolve from the workspace root even when the command runs
inside a nested application. In comparison, `init --source FILE` imports a snapshot
of the source into `assets/cv.md`.

`$EDITOR` takes precedence over `editor`. Editor commands accept quoted paths and
arguments, for example `editor = "code --wait"`; they run directly without a shell.
`pdf.renderer` is one executable name or absolute path, with no inline arguments.
It must support md-to-pdf's stdin/stdout mode, `--basedir`, and `--config-file`.

### Existing workspaces

Existing user configs continue to work. To adopt local configuration with your
current settings, copy the old config into the workspace as `cvmd.toml` (if that
file does not already exist), then run from that workspace:

```sh
cvmd config root .
cvmd info
```

This also corrects any `root` that was relative to the old config directory. Source
and renderer config paths remain relative to the same workspace. Keep the old config
if you still want its fallback outside the workspace. Alternatively, run
`cvmd init ~/CVs` without `--source` to add a config with default settings. It
preserves existing CVs, stylesheets, renderer configs, and applications, and creates
missing asset files from the bundled templates.

## PDF styling

Use `cvmd build --verbose ALIAS` (or `-v`) to print the selected config file and
the absolute Markdown, renderer config, and PDF output paths to stderr. Normal output
still prints `Built …`. Diagnostics include the config selection source and attempted input
paths, including when the renderer config is missing.

`build` renders the selected application's `cv.md`, including your tailored edits.
The configured `source` is copied only when `new` creates an application; changing
it does not replace an existing application's CV. The renderer config path is
resolved on every build, including paths outside the workspace.

### Renderer configuration and columns

New workspaces include `assets/pdf.config.js` and `assets/columns.css`
beside `assets/cv.css`. These shared rendering assets belong together in `assets/`;
they are copied once by `init`, rather than into every application. Existing files
are preserved. If the rendering assets grow, group the config and both stylesheets
under `assets/pdf/` and update `pdf.config_file`.

`build` passes the absolute `pdf.config_file` path to md-to-pdf using
`--config-file`. The JavaScript config controls all stylesheets, PDF options, and
Marked extensions.

The bundled config resolves its CSS paths using `__dirname`, so rendering works
from any application directory and after moving the workspace. When writing your
own JavaScript config, resolve local CSS paths relative to that file as well:

```js
const { resolve } = require("node:path");

module.exports = {
  stylesheet: [resolve(__dirname, "cv.css"), resolve(__dirname, "columns.css")],
  pdf_options: { format: "A4", margin: "10mm", printBackground: true },
  // Keep the bundled marked_extensions to enable the column syntax below.
};
```

Set a custom renderer config with:

```sh
cvmd config pdf.config_file assets/custom-pdf.config.js
cvmd build acme_r_d_a_s
```

For existing workspaces, copy `assets/pdf.config.js` and `assets/columns.css`
from this repository beside your workspace's `assets/cv.css`, then set
`pdf.config_file` as above (using `assets/pdf.config.js` for the bundled file).
If your CSS has an `@page` margin, remove that declaration to let the config's
10 mm margin take effect. Every build requires a renderer config file. Omitting
or unsetting `pdf.config_file` selects `assets/pdf.config.js`.

The bundled Marked extensions support two columns containing ordinary Markdown:

```markdown
:::: columns
::: left-column
## Experience
- PHP and TypeScript development
:::
::: right-column
## Skills
- Rust
:::
::::
```

The outer fence uses four colons; the inner fences use three. Each fence must be
on its own line. Use `pdf_options` for paper size and margins and CSS for layout,
fonts, and spacing. Keep renderer output on stdout (`dest` unset) so `cvmd` can
publish successful PDFs safely; JSON configs are supported by md-to-pdf too,
but the bundled Marked extension functions require JavaScript.

### PDF output

`build` creates an `output/` subdirectory inside the selected application and
saves its PDFs there. The PDF uses the Markdown file's stem by default:
`cv.md` produces `output/cv.pdf`.
To choose a different name, set `filename` under `[pdf]` in `cvmd.toml`:

```toml
[pdf]
filename = "Jane Doe CV.pdf"
```

The name must include the `.pdf` extension and contain no directory components.
Spaces and Unicode are supported. For example, this setting produces
`output/Jane Doe CV.pdf`. It changes the output name without renaming the Markdown.
Remove the setting or run `cvmd config --local --unset pdf.filename` to restore
the default. Changing the setting leaves PDFs with other names intact.

Existing PDFs beside the Markdown are left in place. Future builds write to
`output/`; update any scripts that expect PDFs directly in the application
directory. The renderer still uses the application directory for relative images
and other Markdown assets.

Customize `assets/cv.css` once for all applications, including your fonts, spacing,
and page breaks. `build` sends the application's Markdown to md-to-pdf on stdin,
with the application directory as the base for relative images. Successful output
replaces the selected PDF output; a failed render leaves any existing PDF intact.
Diagnostics from the renderer appear on stderr. Avoid setting `dest` in Markdown front matter,
since cvmd expects the renderer to return the PDF on stdout.

## Development

Run all development checks with Make:

```sh
make check
```

Use `make fmt-check`, `make lint`, or `make test` to run formatting, Clippy,
or tests individually.

Use `make build-dev` to build without installing, `make cli-help` to inspect CLI
usage, and `make fmt` to apply formatting. `make init-dev` creates a fresh isolated
workspace and config under `/tmp` and prints their paths.

Integration tests use isolated workspaces and stub editor/renderer executables,
so the test suite does not require Chromium or open an editor. Process integration
tests using shell scripts run on Unix. CI runs the checks above on Linux.

The MVP includes `init`, `new`, `edit`, `build`, `info`, `config`, and alias selection for
editing/building. Standalone listing, advanced filtering, bulk builds, application
tracking, multiple templates, and AI integration are deferred.

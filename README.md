# Extscan

Extscan is a small command-line tool that reports how many files exist for each file extension in the current directory, along with the size and share of disk usage per extension.

By default, it scans only the directory where the command is executed. Use `--include-sub-dir` to include files inside subdirectories.

## Features

- Counts files grouped by extension.
- Shows total size and percentage of disk usage by extension.
- Scans subdirectories in parallel, with one worker per CPU core.
- Skips unreadable entries with a warning instead of aborting the scan.
- Reports the scan duration.
- Pages the report when it has more than 10 extensions, with arrow-key navigation.
- Filters extensions by substring or glob pattern, from the command line or live inside the pager.
- Shows a side summary panel (grand totals, top 3 extensions by size, filter state) when the terminal is wide enough.
- Sorts extensions alphabetically and prints a running total row on every page.
- Skips hidden files and directories (names starting with `.`) unless `--include-hidden` is given.
- Ignores files without an extension.

## Requirements

- Linux

Rust is required only if you want to build from source. A prebuilt release package can be installed without Rust.

## Install From A Release Package

Download and extract the Linux release package:

```bash
tar -xzf extscan-linux-x86_64.tar.gz
cd extscan-linux-x86_64
bash install.sh
```

The release package includes a prebuilt `extscan` binary, so the install script only copies it to:

```text
~/.local/bin/extscan
```

Make sure `~/.local/bin` is in your `PATH`. If it is not, add this line to your shell configuration file, such as `~/.bashrc` or `~/.zshrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Reload your shell configuration:

```bash
source ~/.bashrc
```

Then run `extscan` from any directory:

```bash
extscan
extscan --include-sub-dir
```

To install into a custom directory, set `INSTALL_DIR`:

```bash
INSTALL_DIR="$HOME/bin" bash install.sh
```

## Build From Source

Install Rust first, then check if it is available:

```bash
rustc --version
cargo --version
```

Build with Cargo:

```bash
cargo build --release
```

Run it from the project directory:

```bash
./target/release/extscan
./target/release/extscan --include-sub-dir
```

You can also compile directly with `rustc`:

```bash
rustc main.rs -o extscan
```

## Install From Source

Run the installation script:

```bash
bash install.sh
```

When running from the source tree, the script compiles `main.rs` and installs the executable at:

```text
~/.local/bin/extscan
```

## Usage

```text
extscan [--include-sub-dir] [--include-hidden] [--no-pager] [--filter <term>]
```

| Flag | Effect |
|---|---|
| `--include-sub-dir` | Scan subdirectories too, not just the current directory. |
| `--include-hidden` | Also count hidden files and descend into hidden directories. By default anything whose name starts with `.` is skipped. |
| `--no-pager` | Print the whole report at once, even when it is long. |
| `--filter <term>` | Show only extensions matching the term (see Filtering below). |

Scan only the current directory:

```bash
extscan
```

Scan the current directory and all subdirectories:

```bash
extscan --include-sub-dir
```

Example output:

```text
Extension    Files       Size     Usage
───────────────────────────────────────
md               3    24.6 KB      1.7%
rs              10     1.4 MB     96.5%
toml             2    25.3 KB      1.7%
───────────────────────────────────────
TOTAL           15     1.4 MB    100.0%

/home/user/project · subdirectories · 2ms
```

The footer shows the scanned directory, the scope, and how long the scan took.

## Pager

When the report has more than 10 extensions and the output is a terminal, extscan shows it page by page, redrawing in place. Every page repeats the header and ends with a running `TOTAL` row that sums everything shown up to that page, so the last page carries the full total.

Keys:

| Key | Action |
|---|---|
| `→` or `Enter` | Next page (finishes on the last page) |
| `←` or `b` | Previous page |
| `/` | Live search: the table filters while you type; `Enter` applies, `Esc` cancels, `Backspace` edits |
| `c` | Clear the active filter |
| `q` | Quit |

When the terminal is wide enough, a side panel stays next to the table with the grand totals of the scan, the three largest extensions by size, and the current filter.

Arrow keys need raw terminal mode, which extscan enables through `stty`. When `stty` is unavailable, the pager falls back to line input: `Enter` advances, `b` goes back, `/term` filters, a bare `/` clears the filter, and `q` quits.

Use `--no-pager` to skip all of this and print the full report in one go, which is also what happens automatically when the output is piped.

## Filtering

Filters apply to extension names and are always case-insensitive:

- A plain term matches as a substring: `mp` matches both `mp3` and `bmp`.
- `.mp3` or `*.mp3` matches the extension exactly.
- `*` matches any sequence of characters: `m*` matches every extension starting with `m`.

Filtering works both up front with `--filter` and interactively with `/` inside the pager. A filtered view keeps its usage percentages relative to the whole scan:

```bash
extscan --include-sub-dir --filter '*.toml'
```

```text
Extension    Files       Size    Usage
──────────────────────────────────────
toml             2    25.3 KB     1.7%
──────────────────────────────────────
TOTAL            2    25.3 KB     1.7%

/home/user/project · subdirectories · 1ms
```

## Uninstall

Remove the installed executable:

```bash
rm "$HOME/.local/bin/extscan"
```

## License

This project is open source and available under the MIT License. See [LICENSE](LICENSE).

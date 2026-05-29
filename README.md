# Extscan

Extscan is a small command-line tool that reports how many files exist for each file extension in the current directory.

By default, it scans only the directory where the command is executed. Use `--include-sub-dir` to include files inside subdirectories.

## Features

- Counts files grouped by extension.
- Shows total size by extension.
- Shows percentage of disk usage by extension.
- Prints a final total row.
- Sorts extensions alphabetically.
- Ignores files without an extension.
- Can scan only the current directory or include subdirectories.

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
File extension report
Directory: /home/user/project
Scope: current directory + subdirectories

+-----------+-------+----------+--------+
| Extension | Files |     Size |  Usage |
+-----------+-------+----------+--------+
| md        |     3 |  12.4 KB |   4.2% |
| rs        |    10 | 256.8 KB |  87.2% |
| toml      |     2 |  25.3 KB |   8.6% |
+-----------+-------+----------+--------+
| TOTAL     |    15 | 294.5 KB | 100.0% |
+-----------+-------+----------+--------+
```

## GitHub Actions

The workflow at `.github/workflows/rust.yml` builds the project with Cargo, runs tests, and uploads a Linux `x86_64` release package artifact.

## Uninstall

Remove the installed executable:

```bash
rm "$HOME/.local/bin/extscan"
```

## License

This project is open source and available under the MIT License. See [LICENSE](LICENSE).

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
- Rust compiler (`rustc`)

Check if Rust is installed:

```bash
rustc --version
```

## Build

Compile the program manually with:

```bash
rustc main.rs -o extscan
```

Run it from the project directory:

```bash
./extscan
./extscan --include-sub-dir
```

## Install For The Current User

Run the installation script:

```bash
./install.sh
```

The script compiles `main.rs` and installs the executable at:

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
INSTALL_DIR="$HOME/bin" ./install.sh
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

## Uninstall

Remove the installed executable:

```bash
rm "$HOME/.local/bin/extscan"
```

## License

This project is open source and available under the MIT License. See [LICENSE](LICENSE).

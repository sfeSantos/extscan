use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let program_name = env::args()
        .next()
        .and_then(|value| {
            Path::new(&value)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "extscan".to_string());
    let config = match Config::from_args(env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("Usage: {} [--include-sub-dir]", program_name);
            std::process::exit(1);
        }
    };

    let directory = env::current_dir()?;
    let mut extension_stats = BTreeMap::new();
    collect_extension_stats(&directory, config.include_sub_dir, &mut extension_stats)?;

    print_report(&directory, config.include_sub_dir, &extension_stats);

    Ok(())
}

struct Config {
    include_sub_dir: bool,
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut include_sub_dir = false;

        for arg in args {
            if arg == "--include-sub-dir" {
                include_sub_dir = true;
            } else if arg.starts_with("--") {
                return Err(format!("Unknown argument: {arg}"));
            } else {
                return Err(format!("Unexpected argument: {arg}"));
            }
        }

        Ok(Self { include_sub_dir })
    }
}

#[derive(Default)]
struct ExtensionStats {
    files: usize,
    bytes: u64,
}

impl ExtensionStats {
    fn add_file(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes += bytes;
    }
}

fn collect_extension_stats(
    directory: &Path,
    include_sub_dir: bool,
    extension_stats: &mut BTreeMap<String, ExtensionStats>,
) -> io::Result<()> {
    let mut directories = vec![PathBuf::from(directory)];

    while let Some(current_directory) = directories.pop() {
        for entry in fs::read_dir(current_directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();

            if file_type.is_file() || file_type.is_symlink() {
                if let Some(extension) = path
                    .extension()
                    .map(|value| value.to_string_lossy().into_owned())
                {
                    if !extension.is_empty() {
                        let metadata = fs::metadata(&path)?;

                        if metadata.is_file() {
                            extension_stats
                                .entry(extension)
                                .or_default()
                                .add_file(metadata.len());
                        }
                    }
                }
            } else if include_sub_dir && file_type.is_dir() {
                directories.push(path);
            }
        }
    }

    Ok(())
}

struct ReportRow {
    extension: String,
    files: usize,
    size: String,
    usage: String,
}

fn print_report(
    directory: &Path,
    include_sub_dir: bool,
    extension_stats: &BTreeMap<String, ExtensionStats>,
) {
    let total_files: usize = extension_stats.values().map(|stats| stats.files).sum();
    let total_bytes: u64 = extension_stats.values().map(|stats| stats.bytes).sum();
    let rows: Vec<ReportRow> = extension_stats
        .iter()
        .map(|(extension, stats)| ReportRow {
            extension: extension.clone(),
            files: stats.files,
            size: format_size(stats.bytes),
            usage: format_usage(stats.bytes, total_bytes),
        })
        .collect();

    let total_size = format_size(total_bytes);
    let total_usage = if total_files == 0 { "0.0%" } else { "100.0%" };

    let extension_width = rows
        .iter()
        .map(|row| row.extension.len())
        .max()
        .unwrap_or(0)
        .max("Extension".len())
        .max("(none)".len())
        .max("TOTAL".len());
    let files_width = rows
        .iter()
        .map(|row| row.files.to_string().len())
        .max()
        .unwrap_or(0)
        .max(total_files.to_string().len())
        .max("Files".len());
    let size_width = rows
        .iter()
        .map(|row| row.size.len())
        .max()
        .unwrap_or(0)
        .max(total_size.len())
        .max("Size".len());
    let usage_width = rows
        .iter()
        .map(|row| row.usage.len())
        .max()
        .unwrap_or(0)
        .max(total_usage.len())
        .max("Usage".len());
    let separator = format!(
        "+-{:-<extension_width$}-+-{:-<files_width$}-+-{:-<size_width$}-+-{:-<usage_width$}-+",
        "",
        "",
        "",
        "",
        extension_width = extension_width,
        files_width = files_width,
        size_width = size_width,
        usage_width = usage_width
    );

    println!("File extension report");
    println!("Directory: {}", directory.display());
    println!(
        "Scope: {}",
        if include_sub_dir {
            "current directory + subdirectories"
        } else {
            "current directory only"
        }
    );
    println!();

    println!("{separator}");
    println!(
        "| {:<extension_width$} | {:>files_width$} | {:>size_width$} | {:>usage_width$} |",
        "Extension",
        "Files",
        "Size",
        "Usage",
        extension_width = extension_width,
        files_width = files_width,
        size_width = size_width,
        usage_width = usage_width
    );
    println!("{separator}");

    if rows.is_empty() {
        println!(
            "| {:<extension_width$} | {:>files_width$} | {:>size_width$} | {:>usage_width$} |",
            "(none)",
            0,
            "0 B",
            "0.0%",
            extension_width = extension_width,
            files_width = files_width,
            size_width = size_width,
            usage_width = usage_width
        );
    } else {
        for row in &rows {
            println!(
                "| {:<extension_width$} | {:>files_width$} | {:>size_width$} | {:>usage_width$} |",
                row.extension,
                row.files,
                row.size,
                row.usage,
                extension_width = extension_width,
                files_width = files_width,
                size_width = size_width,
                usage_width = usage_width
            );
        }
    }

    println!("{separator}");
    println!(
        "| {:<extension_width$} | {:>files_width$} | {:>size_width$} | {:>usage_width$} |",
        "TOTAL",
        total_files,
        total_size,
        total_usage,
        extension_width = extension_width,
        files_width = files_width,
        size_width = size_width,
        usage_width = usage_width
    );
    println!("{separator}");
}

fn format_usage(bytes: u64, total_bytes: u64) -> String {
    if total_bytes == 0 {
        return "0.0%".to_string();
    }

    format!("{:.1}%", bytes as f64 * 100.0 / total_bytes as f64)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
    }
}

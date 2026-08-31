use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::thread;

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
    let extension_stats = collect_extension_stats(&directory, config.include_sub_dir)?;

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

    fn merge(&mut self, other: &ExtensionStats) {
        self.files += other.files;
        self.bytes += other.bytes;
    }
}

fn collect_extension_stats(
    directory: &Path,
    include_sub_dir: bool,
) -> io::Result<BTreeMap<String, ExtensionStats>> {
    let queue = ScanQueue::new(directory.to_path_buf());
    let worker_count = thread::available_parallelism().map_or(1, usize::from);

    let worker_results: Vec<io::Result<HashMap<String, ExtensionStats>>> = thread::scope(|scope| {
        let workers: Vec<_> = (0..worker_count)
            .map(|_| scope.spawn(|| scan_worker(&queue, include_sub_dir)))
            .collect();

        workers
            .into_iter()
            .map(|worker| worker.join().expect("scan worker panicked"))
            .collect()
    });

    let mut extension_stats = BTreeMap::new();
    for worker_stats in worker_results {
        for (extension, stats) in worker_stats? {
            extension_stats
                .entry(extension)
                .or_insert_with(ExtensionStats::default)
                .merge(&stats);
        }
    }

    Ok(extension_stats)
}

/// Shared queue of directories still to be scanned.
///
/// `pending` counts directories that are queued or being processed. The scan
/// is complete when it reaches zero, which is why it lives under the same
/// mutex as the queue: checking "queue empty and nothing in flight" must be
/// atomic.
struct ScanQueue {
    state: Mutex<ScanState>,
    work_available: Condvar,
}

struct ScanState {
    directories: Vec<PathBuf>,
    pending: usize,
    failed: bool,
}

impl ScanQueue {
    fn new(root: PathBuf) -> Self {
        Self {
            state: Mutex::new(ScanState {
                directories: vec![root],
                pending: 1,
                failed: false,
            }),
            work_available: Condvar::new(),
        }
    }

    /// Blocks until a directory is available. Returns `None` when the scan is
    /// complete or another worker reported an error.
    fn next_directory(&self) -> Option<PathBuf> {
        let mut state = self.state.lock().unwrap();

        loop {
            if state.failed || state.pending == 0 {
                return None;
            }

            if let Some(directory) = state.directories.pop() {
                return Some(directory);
            }

            state = self.work_available.wait(state).unwrap();
        }
    }

    fn push_directory(&self, directory: PathBuf) {
        let mut state = self.state.lock().unwrap();
        state.pending += 1;
        state.directories.push(directory);
        self.work_available.notify_one();
    }

    fn finish_directory(&self) {
        let mut state = self.state.lock().unwrap();
        state.pending -= 1;

        if state.pending == 0 {
            self.work_available.notify_all();
        }
    }

    fn fail(&self) {
        let mut state = self.state.lock().unwrap();
        state.failed = true;
        self.work_available.notify_all();
    }
}

fn scan_worker(
    queue: &ScanQueue,
    include_sub_dir: bool,
) -> io::Result<HashMap<String, ExtensionStats>> {
    let mut extension_stats = HashMap::new();

    while let Some(directory) = queue.next_directory() {
        let result = scan_directory(&directory, include_sub_dir, queue, &mut extension_stats);
        queue.finish_directory();

        if let Err(error) = result {
            queue.fail();
            return Err(error);
        }
    }

    Ok(extension_stats)
}

fn scan_directory(
    directory: &Path,
    include_sub_dir: bool,
    queue: &ScanQueue,
    extension_stats: &mut HashMap<String, ExtensionStats>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
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
            queue.push_directory(path);
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!("extscan-test-{name}-{}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn create_file(&self, relative_path: &str, bytes: usize) {
            let path = self.root.join(relative_path);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            fs::write(path, vec![b'x'; bytes]).unwrap();
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn scans_current_directory_only() {
        let tree = TempTree::new("current-only");
        tree.create_file("a.txt", 3);
        tree.create_file("b.txt", 5);
        tree.create_file("c.rs", 7);
        tree.create_file("sub/d.txt", 11);

        let stats = collect_extension_stats(&tree.root, false).unwrap();

        assert_eq!(stats.len(), 2);
        assert_eq!(stats["txt"].files, 2);
        assert_eq!(stats["txt"].bytes, 8);
        assert_eq!(stats["rs"].files, 1);
        assert_eq!(stats["rs"].bytes, 7);
    }

    #[test]
    fn scans_subdirectories_when_enabled() {
        let tree = TempTree::new("recursive");
        tree.create_file("a.txt", 3);
        tree.create_file("sub/b.txt", 5);
        tree.create_file("sub/deeper/c.txt", 11);
        tree.create_file("sub/deeper/d.rs", 7);

        let stats = collect_extension_stats(&tree.root, true).unwrap();

        assert_eq!(stats.len(), 2);
        assert_eq!(stats["txt"].files, 3);
        assert_eq!(stats["txt"].bytes, 19);
        assert_eq!(stats["rs"].files, 1);
        assert_eq!(stats["rs"].bytes, 7);
    }

    #[test]
    fn ignores_files_without_extension() {
        let tree = TempTree::new("no-extension");
        tree.create_file("README", 3);
        tree.create_file(".gitignore", 5);
        tree.create_file("a.txt", 7);

        let stats = collect_extension_stats(&tree.root, true).unwrap();

        assert_eq!(stats.len(), 1);
        assert_eq!(stats["txt"].files, 1);
    }

    #[test]
    fn fails_on_missing_directory() {
        let missing = env::temp_dir().join(format!("extscan-test-missing-{}", std::process::id()));

        assert!(collect_extension_stats(&missing, true).is_err());
    }
}

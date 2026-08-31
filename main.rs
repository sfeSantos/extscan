use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
            eprintln!("Usage: {} [--include-sub-dir] [--no-pager]", program_name);
            std::process::exit(1);
        }
    };

    let directory = env::current_dir()?;
    let start = Instant::now();
    let extension_stats = collect_extension_stats(&directory, config.include_sub_dir)?;
    let duration = start.elapsed();

    print_report(&directory, &config, duration, &extension_stats);

    Ok(())
}

struct Config {
    include_sub_dir: bool,
    no_pager: bool,
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut include_sub_dir = false;
        let mut no_pager = false;

        for arg in args {
            if arg == "--include-sub-dir" {
                include_sub_dir = true;
            } else if arg == "--no-pager" {
                no_pager = true;
            } else if arg.starts_with("--") {
                return Err(format!("Unknown argument: {arg}"));
            } else {
                return Err(format!("Unexpected argument: {arg}"));
            }
        }

        Ok(Self {
            include_sub_dir,
            no_pager,
        })
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
) -> io::Result<HashMap<String, ExtensionStats>> {
    // Only the root is fatal; unreadable entries below it are skipped with a
    // warning during the scan.
    fs::read_dir(directory).map(drop)?;

    let queue = ScanQueue::new(directory.to_path_buf());
    let worker_count = thread::available_parallelism().map_or(1, usize::from);

    let worker_results: Vec<HashMap<String, ExtensionStats>> = thread::scope(|scope| {
        let workers: Vec<_> = (0..worker_count)
            .map(|_| scope.spawn(|| scan_worker(&queue, include_sub_dir)))
            .collect();

        workers
            .into_iter()
            .map(|worker| worker.join().expect("scan worker panicked"))
            .collect()
    });

    let mut extension_stats = HashMap::new();
    for worker_stats in worker_results {
        for (extension, stats) in worker_stats {
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
}

impl ScanQueue {
    fn new(root: PathBuf) -> Self {
        Self {
            state: Mutex::new(ScanState {
                directories: vec![root],
                pending: 1,
            }),
            work_available: Condvar::new(),
        }
    }

    /// Blocks until a directory is available. Returns `None` when the scan is
    /// complete.
    fn next_directory(&self) -> Option<PathBuf> {
        let mut state = self.state.lock().unwrap();

        loop {
            if state.pending == 0 {
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
}

fn scan_worker(queue: &ScanQueue, include_sub_dir: bool) -> HashMap<String, ExtensionStats> {
    let mut extension_stats = HashMap::new();

    while let Some(directory) = queue.next_directory() {
        scan_directory(&directory, include_sub_dir, queue, &mut extension_stats);
        queue.finish_directory();
    }

    extension_stats
}

fn scan_directory(
    directory: &Path,
    include_sub_dir: bool,
    queue: &ScanQueue,
    extension_stats: &mut HashMap<String, ExtensionStats>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("warning: cannot read {}: {error}", directory.display());
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_file() || file_type.is_symlink() {
            // Only the name is needed for the extension; building the full
            // entry path here would allocate once per file for nothing.
            let file_name = entry.file_name();
            let Some(extension) = Path::new(&file_name).extension() else {
                continue;
            };
            let extension = extension.to_string_lossy();

            if extension.is_empty() {
                continue;
            }

            // Regular files get their size straight from the entry, one
            // syscall with no path re-resolution. Only symlinks need the
            // following stat to size the target. Either call fails for
            // dangling symlinks and files removed mid-scan; skip those
            // instead of aborting.
            let metadata = if file_type.is_file() {
                entry.metadata()
            } else {
                fs::metadata(entry.path())
            };

            if let Ok(metadata) = metadata {
                if metadata.is_file() {
                    // Borrowed lookup first: the extension String is only
                    // allocated the first time a worker sees it.
                    match extension_stats.get_mut(extension.as_ref()) {
                        Some(stats) => stats.add_file(metadata.len()),
                        None => {
                            let mut stats = ExtensionStats::default();
                            stats.add_file(metadata.len());
                            extension_stats.insert(extension.into_owned(), stats);
                        }
                    }
                }
            }
        } else if include_sub_dir && file_type.is_dir() {
            queue.push_directory(entry.path());
        }
    }
}

struct ReportRow {
    extension: String,
    files: usize,
    size: String,
    usage: String,
}

const PAGE_SIZE: usize = 10;

fn print_report(
    directory: &Path,
    config: &Config,
    duration: Duration,
    extension_stats: &HashMap<String, ExtensionStats>,
) {
    let use_pager = !config.no_pager
        && extension_stats.len() > PAGE_SIZE
        && io::stdout().is_terminal()
        && io::stdin().is_terminal();

    if !use_pager {
        print!(
            "{}",
            render_report(directory, config.include_sub_dir, duration, extension_stats)
        );
        return;
    }

    let pages = render_report_pages(
        directory,
        config.include_sub_dir,
        duration,
        extension_stats,
        Some(PAGE_SIZE),
    );
    let mut wait_between_pages = true;

    for (index, page) in pages.iter().enumerate() {
        print!("{page}");

        if index + 1 == pages.len() {
            break;
        }

        if wait_between_pages {
            print!(
                "page {}/{} · Enter: next · q: quit ",
                index + 1,
                pages.len()
            );
            let _ = io::stdout().flush();

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                // EOF or a read error: no way to interact, print the rest.
                Ok(0) | Err(_) => wait_between_pages = false,
                Ok(_) => {
                    if input.trim_start().starts_with(['q', 'Q']) {
                        return;
                    }
                }
            }
        }
    }
}

fn render_report(
    directory: &Path,
    include_sub_dir: bool,
    duration: Duration,
    extension_stats: &HashMap<String, ExtensionStats>,
) -> String {
    render_report_pages(directory, include_sub_dir, duration, extension_stats, None)
        .pop()
        .unwrap_or_default()
}

fn render_report_pages(
    directory: &Path,
    include_sub_dir: bool,
    duration: Duration,
    extension_stats: &HashMap<String, ExtensionStats>,
    page_size: Option<usize>,
) -> Vec<String> {
    const COLUMN_GAP: usize = 4;

    let total_files: usize = extension_stats.values().map(|stats| stats.files).sum();
    let total_bytes: u64 = extension_stats.values().map(|stats| stats.bytes).sum();
    let mut rows: Vec<ReportRow> = extension_stats
        .iter()
        .map(|(extension, stats)| ReportRow {
            extension: extension.clone(),
            files: stats.files,
            size: format_size(stats.bytes),
            usage: format_usage(stats.bytes, total_bytes),
        })
        .collect();
    rows.sort_by(|a, b| a.extension.cmp(&b.extension));

    if rows.is_empty() {
        rows.push(ReportRow {
            extension: "(none)".to_string(),
            files: 0,
            size: "0 B".to_string(),
            usage: "0.0%".to_string(),
        });
    }

    let total_size = format_size(total_bytes);
    let total_usage = if total_files == 0 { "0.0%" } else { "100.0%" };

    let extension_width = rows
        .iter()
        .map(|row| row.extension.len())
        .max()
        .unwrap_or(0)
        .max("Extension".len())
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

    let format_row = |extension: &str, files: &str, size: &str, usage: &str| {
        format!(
            "{extension:<extension_width$}{gap}{files:>files_width$}{gap}{size:>size_width$}{gap}{usage:>usage_width$}\n",
            gap = " ".repeat(COLUMN_GAP)
        )
    };
    let separator = format!(
        "{}\n",
        "─".repeat(extension_width + files_width + size_width + usage_width + 3 * COLUMN_GAP)
    );

    let total_row = format_row("TOTAL", &total_files.to_string(), &total_size, total_usage);
    let chunk_size = page_size.unwrap_or(rows.len()).max(1);
    let mut pages = Vec::new();

    // Every page carries the header and the TOTAL row, so each one reads as a
    // complete table on its own.
    for chunk in rows.chunks(chunk_size) {
        let mut page = String::new();
        page.push_str(&format_row("Extension", "Files", "Size", "Usage"));
        page.push_str(&separator);

        for row in chunk {
            page.push_str(&format_row(
                &row.extension,
                &row.files.to_string(),
                &row.size,
                &row.usage,
            ));
        }

        page.push_str(&separator);
        page.push_str(&total_row);
        pages.push(page);
    }

    if let Some(last_page) = pages.last_mut() {
        last_page.push('\n');
        last_page.push_str(&format!(
            "{} · {} · {}\n",
            directory.display(),
            if include_sub_dir {
                "subdirectories"
            } else {
                "current directory"
            },
            format_duration(duration)
        ));
    }

    pages
}

fn format_usage(bytes: u64, total_bytes: u64) -> String {
    if total_bytes == 0 {
        return "0.0%".to_string();
    }

    format!("{:.1}%", bytes as f64 * 100.0 / total_bytes as f64)
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() >= 1 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
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

    #[cfg(unix)]
    #[test]
    fn skips_dangling_symlinks() {
        let tree = TempTree::new("dangling");
        tree.create_file("a.txt", 3);
        std::os::unix::fs::symlink(tree.root.join("missing.so"), tree.root.join("broken.so"))
            .unwrap();

        let stats = collect_extension_stats(&tree.root, true).unwrap();

        assert_eq!(stats.len(), 1);
        assert_eq!(stats["txt"].files, 1);
    }

    #[cfg(unix)]
    #[test]
    fn counts_symlinks_to_existing_files() {
        let tree = TempTree::new("valid-symlink");
        tree.create_file("a.txt", 3);
        std::os::unix::fs::symlink(tree.root.join("a.txt"), tree.root.join("link.txt")).unwrap();

        let stats = collect_extension_stats(&tree.root, false).unwrap();

        assert_eq!(stats["txt"].files, 2);
        assert_eq!(stats["txt"].bytes, 6);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_millis(38)), "38ms");
        assert_eq!(format_duration(Duration::from_millis(1240)), "1.24s");
    }

    #[test]
    fn renders_borderless_report() {
        let mut stats = HashMap::new();
        stats.insert("rs".to_string(), ExtensionStats { files: 1, bytes: 7 });
        stats.insert("txt".to_string(), ExtensionStats { files: 2, bytes: 8 });

        let report = render_report(
            Path::new("/scan/dir"),
            true,
            Duration::from_millis(122),
            &stats,
        );

        let expected = "\
Extension    Files    Size     Usage
────────────────────────────────────
rs               1     7 B     46.7%
txt              2     8 B     53.3%
────────────────────────────────────
TOTAL            3    15 B    100.0%

/scan/dir · subdirectories · 122ms
";
        assert_eq!(report, expected);
    }

    #[test]
    fn paginates_large_reports() {
        let mut stats = HashMap::new();
        for i in 0..25 {
            stats.insert(
                format!("e{i:02}"),
                ExtensionStats {
                    files: 1,
                    bytes: 10,
                },
            );
        }

        let pages = render_report_pages(
            Path::new("/scan/dir"),
            true,
            Duration::from_millis(9),
            &stats,
            Some(10),
        );

        assert_eq!(pages.len(), 3);

        let data_rows = |page: &str| page.lines().filter(|line| line.starts_with('e')).count();
        assert_eq!(data_rows(&pages[0]), 10);
        assert_eq!(data_rows(&pages[1]), 10);
        assert_eq!(data_rows(&pages[2]), 5);

        let rule_len = |page: &str| page.lines().nth(1).unwrap().chars().count();
        for page in &pages {
            assert!(page.starts_with("Extension"));
            assert!(page.contains("TOTAL"));
            assert_eq!(rule_len(page), rule_len(&pages[0]));
        }

        assert!(!pages[0].contains('·'));
        assert!(pages[2].ends_with("/scan/dir · subdirectories · 9ms\n"));
    }

    #[test]
    fn small_report_fits_one_page() {
        let mut stats = HashMap::new();
        stats.insert("rs".to_string(), ExtensionStats { files: 1, bytes: 7 });

        let pages = render_report_pages(
            Path::new("/scan/dir"),
            false,
            Duration::from_millis(5),
            &stats,
            Some(10),
        );

        assert_eq!(pages.len(), 1);
        assert_eq!(
            pages[0],
            render_report(
                Path::new("/scan/dir"),
                false,
                Duration::from_millis(5),
                &stats
            )
        );
    }

    #[test]
    fn renders_empty_report() {
        let stats = HashMap::new();

        let report = render_report(
            Path::new("/scan/dir"),
            false,
            Duration::from_millis(5),
            &stats,
        );

        let expected = "\
Extension    Files    Size    Usage
───────────────────────────────────
(none)           0     0 B     0.0%
───────────────────────────────────
TOTAL            0     0 B     0.0%

/scan/dir · current directory · 5ms
";
        assert_eq!(report, expected);
    }
}

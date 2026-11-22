//! Tells the library when something changes: files in a library folder, or a drive coming or going.
//! macOS file events don't hold files open, so watching a drive never stops it ejecting.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

pub struct LibraryWatcher {
    watcher: RecommendedWatcher,
    watched: Vec<PathBuf>,
}

impl LibraryWatcher {
    /// Starts watching /Volumes. `on_change` runs on a background thread once changes have settled.
    pub fn start(on_change: impl Fn() + Send + 'static) -> Result<Self, String> {
        let quiet = Duration::from_secs(2);
        let longest_wait = Duration::from_secs(60);

        let (changed, changes) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let Ok(event) = event else {
                return;
            };
            if event.paths.iter().any(|path| !is_hidden(path)) {
                let _ = changed.send(());
            }
        })
        .map_err(|err| format!("Can't start watching for changes: {err}"))?;
        watcher
            .watch(Path::new("/Volumes"), RecursiveMode::NonRecursive)
            .map_err(|err| format!("Can't watch for drives: {err}"))?;

        thread::spawn(move || run_when_settled(&changes, quiet, longest_wait, on_change));
        Ok(LibraryWatcher {
            watcher,
            watched: Vec::new(),
        })
    }

    /// Watches exactly these library folders (and everything inside them), dropping any others.
    pub fn watch_folders(&mut self, folders: &[PathBuf]) {
        let gone: Vec<PathBuf> = self
            .watched
            .iter()
            .filter(|folder| !folders.contains(folder))
            .cloned()
            .collect();
        for folder in gone {
            // Fails when the drive has already gone, which is fine: there's nothing left to watch.
            let _ = self.watcher.unwatch(&folder);
            self.watched.retain(|watched| *watched != folder);
        }
        for folder in folders {
            if self.watched.contains(folder) {
                continue;
            }
            match self.watcher.watch(folder, RecursiveMode::Recursive) {
                Ok(()) => self.watched.push(folder.clone()),
                Err(err) => eprintln!("Can't watch a library folder: {err}"),
            }
        }
    }
}

/// For every burst of changes, waits for it to go quiet, then calls `on_change` once.
/// Ends when the watcher is dropped.
fn run_when_settled(
    changes: &Receiver<()>,
    quiet: Duration,
    longest_wait: Duration,
    on_change: impl Fn(),
) {
    for () in changes.iter() {
        wait_until_quiet(changes, quiet, longest_wait);
        on_change();
    }
}

/// Returns after `quiet` with no new change, or after `longest_wait` at most (a long copy keeps changing).
fn wait_until_quiet(changes: &Receiver<()>, quiet: Duration, longest_wait: Duration) {
    let started = Instant::now();
    while started.elapsed() < longest_wait {
        match changes.recv_timeout(quiet) {
            Ok(()) => continue,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Finder's `.DS_Store`, macOS `._` copies and tapehead's own folders don't change the library.
fn is_hidden(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn count_runs(send_events: impl FnOnce(&mpsc::Sender<()>)) -> usize {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let (changed, changes) = mpsc::channel();
        let settler = thread::spawn(move || {
            run_when_settled(
                &changes,
                Duration::from_millis(50),
                Duration::from_millis(500),
                move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                },
            )
        });
        send_events(&changed);
        drop(changed);
        settler.join().unwrap();
        runs.load(Ordering::SeqCst)
    }

    #[test]
    fn a_burst_of_changes_runs_once() {
        let runs = count_runs(|changed| {
            for _ in 0..20 {
                changed.send(()).unwrap();
            }
        });
        assert_eq!(runs, 1);
    }

    #[test]
    fn two_bursts_apart_run_twice() {
        let runs = count_runs(|changed| {
            changed.send(()).unwrap();
            thread::sleep(Duration::from_millis(200));
            changed.send(()).unwrap();
        });
        assert_eq!(runs, 2);
    }

    #[test]
    fn nonstop_changes_still_run_after_the_longest_wait() {
        let runs = count_runs(|changed| {
            for _ in 0..40 {
                changed.send(()).unwrap();
                thread::sleep(Duration::from_millis(20));
            }
        });
        assert!(runs >= 2, "ran {runs} times");
    }

    /// Uses real macOS file events on a temporary folder, so it takes a few seconds.
    #[test]
    fn notices_a_new_file_in_a_watched_folder() {
        let folder = std::env::temp_dir().join(format!("fony-watch-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let folder = std::fs::canonicalize(&folder).unwrap();

        let (ran, runs) = mpsc::channel();
        let mut watcher = LibraryWatcher::start(move || {
            let _ = ran.send(());
        })
        .unwrap();
        watcher.watch_folders(std::slice::from_ref(&folder));
        thread::sleep(Duration::from_millis(500));

        std::fs::write(folder.join("New Episode.mkv"), b"test").unwrap();
        let noticed = runs.recv_timeout(Duration::from_secs(10));
        std::fs::remove_dir_all(&folder).unwrap();
        assert!(noticed.is_ok(), "no change reported within 10 seconds");
    }

    #[test]
    fn ignores_hidden_files() {
        assert!(is_hidden(Path::new(
            "/Volumes/Glitchy_2TB/MEDIA/Cartoons/.DS_Store"
        )));
        assert!(is_hidden(Path::new(
            "/Volumes/Glitchy_2TB/MEDIA/Cartoons/.tapehead/series.yaml"
        )));
        assert!(!is_hidden(Path::new("/Volumes/Glitchy_2TB")));
        assert!(!is_hidden(Path::new(
            "/Volumes/Glitchy_2TB/MEDIA/Cartoons/DuckTales (1987)/Season 01/E1.mkv"
        )));
    }
}

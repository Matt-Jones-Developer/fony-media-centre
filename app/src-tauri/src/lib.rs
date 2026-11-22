mod drives;
mod library;
mod mpv;
mod watch;

use drives::VolumeInfo;
use library::db::{DriveSummary, Library, LibraryCounts};
use library::parse::TidyRules;
use library::probe;
use library::scan;
use mpv::Mpv;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use watch::LibraryWatcher;

/// The one mpv we control. `None` until the first file is played.
/// Tauri commands can run on several threads, so the Mutex makes them take turns.
struct Player(Mutex<Option<Mpv>>);

/// The library database, shared by every command.
struct LibraryState(Mutex<Library>);

fn lock_library<'a>(state: &'a State<LibraryState>) -> Result<MutexGuard<'a, Library>, String> {
    state
        .0
        .lock()
        .map_err(|_| "Library state is broken; restart the app".into())
}

/// True while ffprobe is working through the library.
struct Probing(AtomicBool);

/// Probes files in the background. If probing is already running, this does nothing.
fn start_probing(app: &AppHandle) {
    if app.state::<Probing>().0.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = probe_all(&handle) {
            eprintln!("Probing stopped: {err}");
        }
        handle.state::<Probing>().0.store(false, Ordering::SeqCst);
    });
}

/// Batches of 50, four ffprobes at a time, telling the UI after each batch.
fn probe_all(app: &AppHandle) -> Result<(), String> {
    let batch_size = 50;
    let workers = 4;
    let library = app.state::<LibraryState>();
    let jobs = lock_library(&library)?.files_to_probe(&drives::mounted_volumes())?;
    for batch in jobs.chunks(batch_size) {
        let results = probe::probe_many(batch, workers);
        lock_library(&library)?.save_probes(&results)?;
        app.emit("library-changed", ())
            .map_err(|err| format!("Can't tell the UI the library changed: {err}"))?;
    }
    Ok(())
}

/// Watches /Volumes and the library folders. `None` if watching couldn't start.
struct WatcherState(Mutex<Option<LibraryWatcher>>);

/// Held for the whole of a rescan, so a second one waits instead of running alongside.
struct ScanLock(Mutex<()>);

/// Scans every library folder on every plugged-in drive, tells the UI with `library-changed`,
/// then starts probing whatever is new.
/// The database is only locked while saving, not while walking the drive.
fn rescan_all(app: &AppHandle) -> Result<LibraryCounts, String> {
    let scan_lock = app.state::<ScanLock>();
    let _scanning = scan_lock
        .0
        .lock()
        .map_err(|_| "Scan state is broken; restart the app")?;
    let library = app.state::<LibraryState>();

    let mounted = drives::mounted_volumes();
    let targets = {
        let mut library = lock_library(&library)?;
        library.record_mounted(&mounted)?;
        library.scan_targets(&mounted)?
    };

    // One folder failing (say its drive is pulled out mid-scan) mustn't stop the others or the UI update.
    let rules = TidyRules::new();
    for target in &targets {
        match scan::scan_library_folder(&target.folder, &target.rel_path, &rules) {
            Ok(result) => lock_library(&library)?.save_scan(target, &result)?,
            Err(err) => eprintln!("Skipped a library folder: {err}"),
        }
    }

    let folders: Vec<PathBuf> = targets.iter().map(|target| target.folder.clone()).collect();
    if let Ok(mut watcher) = app.state::<WatcherState>().0.lock()
        && let Some(watcher) = watcher.as_mut()
    {
        watcher.watch_folders(&folders);
    }

    let counts = lock_library(&library)?.counts()?;
    app.emit("library-changed", ())
        .map_err(|err| format!("Can't tell the UI the library changed: {err}"))?;
    start_probing(app);
    Ok(counts)
}

/// Each CRT preset is the crt-lottes shader with different settings.
/// See app/shaders/README.md for what each setting does.
fn crt_preset_options(preset: &str) -> Result<Option<&'static str>, String> {
    match preset {
        "Trinitron" => Ok(Some(
            "SHADOW_MASK=2,HARD_SCAN=-8,CURVATURE_X=0.031,CURVATURE_Y=0.041,MASK_DARK=0.5,MASK_LIGHT=1.5,BLOOM_AMOUNT=0.0625",
        )),
        "Portable" => Ok(Some(
            "SHADOW_MASK=1,HARD_SCAN=-5,CURVATURE_X=0.05,CURVATURE_Y=0.065,MASK_DARK=0.65,MASK_LIGHT=1.35,BLOOM_AMOUNT=0.09",
        )),
        "Arcade" => Ok(Some(
            "SHADOW_MASK=3,HARD_SCAN=-12,CURVATURE_X=0.02,CURVATURE_Y=0.025,MASK_DARK=0.4,MASK_LIGHT=1.6,BLOOM_AMOUNT=0.05",
        )),
        "Clean" => Ok(None),
        _ => Err(format!("Unknown CRT preset: {preset}")),
    }
}

/// Runs `action` against mpv, starting mpv first if it isn't running (or was closed by hand).
fn with_mpv<T>(
    app: &AppHandle,
    player: &State<Player>,
    action: impl FnOnce(&Mpv) -> Result<T, String>,
) -> Result<T, String> {
    let mut slot = player
        .0
        .lock()
        .map_err(|_| "Player state is broken; restart the app")?;
    let needs_start = match slot.as_mut() {
        Some(mpv) => !mpv.is_running(),
        None => true,
    };
    if needs_start {
        *slot = Some(Mpv::start(app.clone())?);
    }
    let mpv = slot.as_ref().ok_or("mpv didn't start")?;
    action(mpv)
}

#[tauri::command]
fn mpv_load(app: AppHandle, player: State<Player>, path: String) -> Result<(), String> {
    let path = Path::new(&path);
    if !path.is_file() {
        return Err(format!("Not a file: {}", path.display()));
    }
    with_mpv(&app, &player, |mpv| mpv.load(path))
}

#[tauri::command]
fn mpv_toggle_pause(app: AppHandle, player: State<Player>) -> Result<(), String> {
    with_mpv(&app, &player, |mpv| mpv.toggle_pause())
}

#[tauri::command]
fn mpv_seek(app: AppHandle, player: State<Player>, seconds: f64) -> Result<(), String> {
    with_mpv(&app, &player, |mpv| mpv.seek(seconds))
}

#[tauri::command]
fn mpv_stop(app: AppHandle, player: State<Player>) -> Result<(), String> {
    with_mpv(&app, &player, |mpv| mpv.stop())
}

#[tauri::command]
fn mpv_set_crt(app: AppHandle, player: State<Player>, preset: String) -> Result<(), String> {
    let options = crt_preset_options(&preset)?;
    let shader_path = app
        .path()
        .resolve("shaders/crt-lottes.glsl", BaseDirectory::Resource)
        .map_err(|err| format!("Can't find the CRT shader: {err}"))?;
    with_mpv(&app, &player, |mpv| {
        mpv.set_shader(options.map(|opts| (shader_path.as_path(), opts)))
    })
}

#[tauri::command]
fn volume_info(path: String) -> Result<VolumeInfo, String> {
    drives::volume_info(Path::new(&path))
}

/// Adds a folder (e.g. MEDIA/Cartoons) to the library, recording its drive by UUID.
#[tauri::command]
fn library_add_folder(
    library: State<LibraryState>,
    path: String,
    kind: String,
) -> Result<(), String> {
    let folder = fs::canonicalize(&path).map_err(|err| format!("Can't read {path}: {err}"))?;
    let volume = drives::volume_info(&folder)?;
    let rel_path = drives::path_on_drive(&folder, Path::new(&volume.mount_point))?;
    lock_library(&library)?.add_library_folder(&volume, &rel_path, &kind)
}

/// Async so the diskutil calls run off the main thread and don't freeze the window.
#[tauri::command]
async fn library_drives(library: State<'_, LibraryState>) -> Result<Vec<DriveSummary>, String> {
    let mounted = drives::mounted_volumes();
    let mut library = lock_library(&library)?;
    library.record_mounted(&mounted)?;
    library.drives(&mounted)
}

/// Async so the scan runs off the main thread and the window stays responsive.
#[tauri::command]
async fn library_rescan(app: AppHandle) -> Result<LibraryCounts, String> {
    rescan_all(&app)
}

#[tauri::command]
fn library_counts(library: State<LibraryState>) -> Result<LibraryCounts, String> {
    lock_library(&library)?.counts()
}

#[tauri::command]
fn library_review_queue(library: State<LibraryState>) -> Result<Vec<String>, String> {
    lock_library(&library)?.review_queue()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Player(Mutex::new(None)))
        .setup(|app| {
            // Not app_data_dir(): that would name the folder after the bundle id, not the app.
            let db_path = app.path().data_dir()?.join("FonyMediaCentre/library.db");
            app.manage(LibraryState(Mutex::new(Library::open(&db_path)?)));
            app.manage(ScanLock(Mutex::new(())));
            app.manage(Probing(AtomicBool::new(false)));

            let watch_handle = app.handle().clone();
            let watcher = LibraryWatcher::start(move || {
                if let Err(err) = rescan_all(&watch_handle) {
                    eprintln!("Library scan after a change failed: {err}");
                }
            });
            if let Err(err) = &watcher {
                eprintln!("{err}");
            }
            app.manage(WatcherState(Mutex::new(watcher.ok())));

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(err) = rescan_all(&handle) {
                    eprintln!("Library scan on launch failed: {err}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mpv_load,
            mpv_toggle_pause,
            mpv_seek,
            mpv_stop,
            mpv_set_crt,
            volume_info,
            library_add_folder,
            library_drives,
            library_rescan,
            library_counts,
            library_review_queue
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Taking the Mpv out of the slot drops it, and dropping it quits mpv.
                if let Ok(mut slot) = app.state::<Player>().0.lock() {
                    slot.take();
                }
            }
        });
}

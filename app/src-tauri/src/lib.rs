mod mpv;
mod volume;

use mpv::Mpv;
use std::path::Path;
use std::sync::Mutex;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, RunEvent, State};
use volume::VolumeInfo;

/// The one mpv we control. `None` until the first file is played.
/// Tauri commands can run on several threads, so the Mutex makes them take turns.
struct Player(Mutex<Option<Mpv>>);

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
    volume::volume_info(Path::new(&path))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Player(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            mpv_load,
            mpv_toggle_pause,
            mpv_seek,
            mpv_stop,
            mpv_set_crt,
            volume_info
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

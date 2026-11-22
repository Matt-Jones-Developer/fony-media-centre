//! Runs mpv as its own process and talks to it over mpv's JSON IPC socket.
//!
//! The socket is a Unix socket file. We write one JSON command per line, and mpv writes
//! back one JSON message per line. A background thread reads those messages and forwards
//! playback state to the UI as a Tauri event called `mpv-state`.

use serde::Serialize;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};

/// What the UI needs to know about playback. `Option` means "may not be known yet".
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub path: Option<String>,
    pub time_pos: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    pub idle: bool,
    /// Which hardware decoder mpv is using, e.g. "videotoolbox". "no" means software.
    pub hwdec: Option<String>,
}

pub struct Mpv {
    child: Child,
    socket: UnixStream,
    socket_path: PathBuf,
}

impl Mpv {
    /// Starts mpv idle (no file yet) and connects to its socket.
    pub fn start<R: Runtime>(app: AppHandle<R>) -> Result<Mpv, String> {
        let socket_path =
            std::env::temp_dir().join(format!("fony-mpv-{}.sock", std::process::id()));

        let child = Command::new(mpv_binary())
            .arg("--no-config")
            .arg("--idle=yes")
            .arg("--force-window=no")
            .arg("--keep-open=yes")
            .arg("--hwdec=videotoolbox")
            .arg("--no-terminal")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("Couldn't start mpv: {err}"))?;

        let socket = connect(&socket_path)?;
        let mpv = Mpv {
            child,
            socket,
            socket_path,
        };

        let state = Arc::new(Mutex::new(PlayerState {
            idle: true,
            ..Default::default()
        }));
        let reader = mpv.socket.try_clone().map_err(|err| err.to_string())?;
        thread::spawn(move || forward_events(reader, state, app));

        let watched = [
            "path",
            "time-pos",
            "duration",
            "pause",
            "idle-active",
            "hwdec-current",
        ];
        for (id, name) in watched.iter().enumerate() {
            mpv.send(json!(["observe_property", id + 1, name]))?;
        }
        Ok(mpv)
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn load(&self, path: &Path) -> Result<(), String> {
        self.send(json!(["loadfile", path.to_string_lossy(), "replace"]))?;
        self.send(json!(["set", "pause", "no"]))
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        self.send(json!(["cycle", "pause"]))
    }

    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        self.send(json!(["seek", seconds, "relative"]))
    }

    pub fn stop(&self) -> Result<(), String> {
        self.send(json!(["stop"]))
    }

    /// `None` turns the shader off (the Clean preset).
    pub fn set_shader(&self, shader: Option<(&Path, &str)>) -> Result<(), String> {
        let Some((shader_path, options)) = shader else {
            return self.send(json!(["change-list", "glsl-shaders", "clr", ""]));
        };
        self.send(json!(["set", "glsl-shader-opts", options]))?;
        self.send(json!([
            "set",
            "glsl-shaders",
            shader_path.to_string_lossy()
        ]))
    }

    fn send(&self, command: Value) -> Result<(), String> {
        let line = json!({ "command": command }).to_string() + "\n";
        // `&self.socket` lets us write without needing a mutable Mpv.
        (&self.socket)
            .write_all(line.as_bytes())
            .map_err(|err| format!("mpv isn't responding: {err}"))
    }
}

/// Rust runs `drop` automatically when an Mpv goes away (app quit, or replaced after a crash),
/// so mpv can never be left running behind us.
impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.send(json!(["quit"]));
        let polls = 20;
        for _ in 0..polls {
            if !self.is_running() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// A Finder-launched app doesn't get the terminal's PATH, so look in Homebrew's folders first.
fn mpv_binary() -> PathBuf {
    let candidates = ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"];
    for candidate in candidates {
        if Path::new(candidate).exists() {
            return PathBuf::from(candidate);
        }
    }
    PathBuf::from("mpv")
}

/// mpv creates the socket a moment after it starts, so retry for up to 5 seconds.
fn connect(socket_path: &Path) -> Result<UnixStream, String> {
    let attempts = 50;
    for _ in 0..attempts {
        if let Ok(socket) = UnixStream::connect(socket_path) {
            return Ok(socket);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("mpv started but its control socket never appeared".into())
}

fn forward_events<R: Runtime>(
    reader: UnixStream,
    state: Arc<Mutex<PlayerState>>,
    app: AppHandle<R>,
) {
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message["event"] != "property-change" {
            continue;
        }
        let Ok(mut current) = state.lock() else { break };
        apply_change(&mut current, &message["name"], &message["data"]);
        let _ = app.emit("mpv-state", current.clone());
    }
}

fn apply_change(state: &mut PlayerState, name: &Value, data: &Value) {
    match name.as_str() {
        Some("path") => state.path = data.as_str().map(String::from),
        Some("time-pos") => state.time_pos = data.as_f64(),
        Some("duration") => state.duration = data.as_f64(),
        Some("pause") => state.paused = data.as_bool().unwrap_or(false),
        Some("idle-active") => state.idle = data.as_bool().unwrap_or(false),
        Some("hwdec-current") => state.hwdec = data.as_str().map(String::from),
        _ => {}
    }
}

/// Phase 0c acceptance check. It plays real files from the media drive, so it only runs when
/// asked: `cargo test -- --ignored --nocapture`. mpv windows open and close while it runs.
#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Listener;

    const CARTOONS: &str = "/Volumes/Glitchy_2TB/MEDIA/Cartoons";

    fn latest(state: &Arc<Mutex<Value>>) -> Value {
        state.lock().unwrap().clone()
    }

    /// Polls up to 10 seconds for a condition on the latest state from mpv.
    fn wait_for(state: &Arc<Mutex<Value>>, what: &str, check: impl Fn(&Value) -> bool) -> Value {
        let polls = 100;
        for _ in 0..polls {
            let current = latest(state);
            if check(&current) {
                return current;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for {what}; last state: {}",
            latest(state)
        );
    }

    #[test]
    #[ignore]
    fn plays_each_format_with_live_controls_and_cleans_up() {
        let app = tauri::test::mock_app();
        let state = Arc::new(Mutex::new(Value::Null));
        let sink = state.clone();
        app.listen_any("mpv-state", move |event| {
            *sink.lock().unwrap() = serde_json::from_str(event.payload()).unwrap();
        });

        let mpv = Mpv::start(app.handle().clone()).expect("mpv starts");
        let pid = mpv.child.id();
        let shader = Path::new(env!("CARGO_MANIFEST_DIR")).join("../shaders/crt-lottes.glsl");
        let files = [
            "The Smurfs (1981)/Season 01/The Smurfs (1981) - S01E01 - The Astrosmurf.mkv",
            "Little Miss (1983)/Season 01/Little Miss (1983) - S01E01 - Little Miss Splendid.mp4",
            "Rocko's Modern Life (1993)/Season 01/Rocko's Modern Life (1993) - S01E01 - No Pain, No Gain.avi",
            "Dungeons & Dragons (1983)/Season 01/Dungeons & Dragons (1983) - S01E01 - The Night of No Tomorrow.mkv",
        ];

        for file in files {
            let path = Path::new(CARTOONS).join(file);
            mpv.load(&path).unwrap();
            // Early in the new file, so a position left over from the previous file can't pass.
            let playing = wait_for(&state, "playback", |s| {
                let position = s["timePos"].as_f64().unwrap_or(0.0);
                s["path"].as_str() == path.to_str()
                    && (1.0..6.0).contains(&position)
                    && s["hwdec"].is_string()
            });
            let before = playing["timePos"].as_f64().unwrap();

            mpv.seek(10.0).unwrap();
            wait_for(&state, "seek", |s| {
                s["timePos"].as_f64().unwrap_or(0.0) > before + 8.0
            });

            mpv.set_shader(Some((&shader, "SHADOW_MASK=1,HARD_SCAN=-5")))
                .unwrap();
            mpv.set_shader(Some((&shader, "SHADOW_MASK=2,HARD_SCAN=-8")))
                .unwrap();
            mpv.set_shader(None).unwrap();

            mpv.toggle_pause().unwrap();
            wait_for(&state, "pause", |s| s["paused"] == true);
            mpv.toggle_pause().unwrap();
            wait_for(&state, "resume", |s| s["paused"] == false);

            let name = path.file_name().unwrap().to_string_lossy();
            println!("{name}: hwdec = {}", playing["hwdec"]);
        }

        drop(mpv);
        let still_running = Command::new("/bin/ps")
            .args(["-p", &pid.to_string()])
            .status()
            .unwrap()
            .success();
        assert!(
            !still_running,
            "mpv (pid {pid}) is still running after quit"
        );
        println!("mpv quit cleanly");
    }
}

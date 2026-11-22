//! Reads a video's running time, codec and picture size with ffprobe. Only reads the drive.

use super::db::ProbeJob;
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub struct ProbeInfo {
    pub duration_ms: Option<i64>,
    pub video_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_name: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    /// Seconds, as text, e.g. "1294.760000".
    duration: Option<String>,
}

/// Probes `jobs` with `workers` ffprobes running at once. Files that fail are left out,
/// so they stay unprobed and are tried again next time.
pub fn probe_many(jobs: &[ProbeJob], workers: usize) -> Vec<(i64, ProbeInfo)> {
    let chunk_size = jobs.len().div_ceil(workers.max(1)).max(1);
    let mut probed = Vec::new();
    thread::scope(|scope| {
        let running: Vec<_> = jobs
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || probe_chunk(chunk)))
            .collect();
        for worker in running {
            if let Ok(results) = worker.join() {
                probed.extend(results);
            }
        }
    });
    probed
}

fn probe_chunk(jobs: &[ProbeJob]) -> Vec<(i64, ProbeInfo)> {
    let mut results = Vec::new();
    for job in jobs {
        match probe_file(&job.path) {
            Ok(info) => results.push((job.file_id, info)),
            Err(err) => eprintln!("ffprobe skipped a file: {err}"),
        }
    }
    results
}

pub fn probe_file(path: &Path) -> Result<ProbeInfo, String> {
    let child = Command::new(ffprobe_binary())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "format=duration:stream=codec_name,width,height",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Couldn't run ffprobe: {err}"))?;
    let output = wait_for_output(child, path)?;
    parse_output(&output)
}

/// Waits up to 30 seconds, checking every 20 ms. A stuck ffprobe (a drive that stops answering) is stopped.
fn wait_for_output(mut child: Child, path: &Path) -> Result<String, String> {
    let checks = 1500;
    for _ in 0..checks {
        let status = child
            .try_wait()
            .map_err(|err| format!("Lost track of ffprobe: {err}"))?;
        let Some(status) = status else {
            thread::sleep(Duration::from_millis(20));
            continue;
        };
        if !status.success() {
            return Err(format!("ffprobe couldn't read {}", path.display()));
        }
        let mut output = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout
                .read_to_string(&mut output)
                .map_err(|err| format!("Can't read ffprobe's answer: {err}"))?;
        }
        return Ok(output);
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(format!("ffprobe took too long on {}", path.display()))
}

fn parse_output(json: &str) -> Result<ProbeInfo, String> {
    let output: FfprobeOutput = serde_json::from_str(json)
        .map_err(|err| format!("ffprobe's answer can't be read: {err}"))?;
    let video = output.streams.into_iter().next();
    let duration_ms = output
        .format
        .and_then(|format| format.duration)
        .and_then(|seconds| seconds.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        // There's no checked float-to-integer conversion; the filter above keeps this safe.
        .map(|seconds| (seconds * 1000.0).round() as i64);

    let Some(video) = video else {
        return Ok(ProbeInfo {
            duration_ms,
            video_codec: None,
            width: None,
            height: None,
        });
    };
    Ok(ProbeInfo {
        duration_ms,
        video_codec: video.codec_name,
        width: video.width.filter(|width| *width > 0),
        height: video.height.filter(|height| *height > 0),
    })
}

/// A Finder-launched app doesn't get the terminal's PATH, so look in Homebrew's folders first.
fn ffprobe_binary() -> PathBuf {
    let candidates = ["/opt/homebrew/bin/ffprobe", "/usr/local/bin/ffprobe"];
    for candidate in candidates {
        if Path::new(candidate).exists() {
            return PathBuf::from(candidate);
        }
    }
    PathBuf::from("ffprobe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ffprobes_answer() {
        let json = r#"{
            "programs": [], "stream_groups": [],
            "streams": [{ "codec_name": "mpeg4", "width": 640, "height": 480 }],
            "format": { "duration": "1294.760000" }
        }"#;
        assert_eq!(
            parse_output(json).unwrap(),
            ProbeInfo {
                duration_ms: Some(1_294_760),
                video_codec: Some("mpeg4".into()),
                width: Some(640),
                height: Some(480),
            }
        );
    }

    #[test]
    fn copes_with_missing_pieces() {
        let info = parse_output(r#"{ "streams": [], "format": {} }"#).unwrap();
        assert_eq!(info.duration_ms, None);
        assert_eq!(info.video_codec, None);

        let info =
            parse_output(r#"{ "streams": [{ "codec_name": "h264", "width": 0 }], "format": { "duration": "N/A" } }"#)
                .unwrap();
        assert_eq!((info.duration_ms, info.width), (None, None));
    }

    #[test]
    fn refuses_text_that_isnt_json() {
        assert!(parse_output("Invalid data found when processing input").is_err());
    }

    /// Needs the media drive plugged in: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probes_a_real_mkv_mp4_and_avi() {
        let cartoons = Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons");
        let files = [
            "The Smurfs (1981)/Season 01/The Smurfs (1981) - S01E01 - The Astrosmurf.mkv",
            "Little Miss (1983)/Season 01/Little Miss (1983) - S01E01 - Little Miss Splendid.mp4",
            "Rocko's Modern Life (1993)/Season 01/Rocko's Modern Life (1993) - S01E01 - No Pain, No Gain.avi",
        ];
        for file in files {
            let info = probe_file(&cartoons.join(file)).unwrap();
            println!("{file}: {info:?}");
            assert!(info.duration_ms.is_some_and(|ms| ms > 60_000), "{file}");
            assert!(info.video_codec.is_some(), "{file}");
            assert!(info.width.is_some(), "{file}");
        }
    }
}

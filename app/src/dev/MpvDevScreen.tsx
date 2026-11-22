import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import LibraryPanel from "./LibraryPanel";

type PlayerState = {
  path: string | null;
  timePos: number | null;
  duration: number | null;
  paused: boolean;
  idle: boolean;
  hwdec: string | null;
};

type VolumeInfo = {
  mountPoint: string;
  volumeName: string | null;
  volumeUuid: string | null;
};

const config = {
  startFolder: "/Volumes/Glitchy_2TB/MEDIA/Cartoons",
  videoExtensions: ["mkv", "mp4", "avi", "m4v", "mov", "mpg"],
  seekSeconds: 10,
  crtPresets: ["Trinitron", "Portable", "Arcade", "Clean"],
};

function formatTime(seconds: number | null) {
  if (seconds === null) return "--:--:--";
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const secs = whole % 60;
  return [hours, minutes, secs].map((part) => String(part).padStart(2, "0")).join(":");
}

function describeHwdec(hwdec: string | null) {
  if (!hwdec) return "not playing";
  if (hwdec === "no") return "software decoding";
  return hwdec;
}

export default function MpvDevScreen() {
  const [player, setPlayer] = useState<PlayerState | null>(null);
  const [volume, setVolume] = useState<VolumeInfo | null>(null);
  const [crtPreset, setCrtPreset] = useState("Clean");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const stopListening = listen<PlayerState>("mpv-state", (event) => setPlayer(event.payload));
    return () => {
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  async function run(command: string, args?: Record<string, unknown>) {
    setError(null);
    try {
      await invoke(command, args);
    } catch (err) {
      setError(String(err));
    }
  }

  async function pickAndPlay() {
    const path = await open({
      defaultPath: config.startFolder,
      filters: [{ name: "Video", extensions: config.videoExtensions }],
    });
    if (typeof path !== "string") return;

    await run("mpv_load", { path });
    try {
      setVolume(await invoke<VolumeInfo>("volume_info", { path }));
    } catch (err) {
      setError(String(err));
    }
  }

  async function changeCrtPreset(preset: string) {
    setCrtPreset(preset);
    await run("mpv_set_crt", { preset });
  }

  const fileName = player?.path?.split("/").pop() ?? "No tape loaded";
  const isPlaying = player && !player.idle;

  return (
    <main className="min-h-screen bg-ash-dark p-8 font-sans text-paper">
      <h1 className="mb-1 font-mono text-xl tracking-widest">FONY · MPV DEV SCREEN</h1>
      <p className="mb-6 text-sm text-paper-muted">Phase 0c test bench. Playback opens in its own mpv window.</p>

      <LibraryPanel />

      <section aria-label="Transport" className="mb-6 flex flex-wrap gap-2">
        <button type="button" className="rounded bg-dymo-blue px-4 py-2 font-mono font-bold" onClick={pickAndPlay}>
          Pick file and play
        </button>
        <button type="button" className="rounded bg-plastic px-4 py-2 font-mono" onClick={() => run("mpv_toggle_pause")}>
          Play / Pause
        </button>
        <button
          type="button"
          className="rounded bg-plastic px-4 py-2 font-mono"
          onClick={() => run("mpv_seek", { seconds: -config.seekSeconds })}
        >
          Back 10 s
        </button>
        <button
          type="button"
          className="rounded bg-plastic px-4 py-2 font-mono"
          onClick={() => run("mpv_seek", { seconds: config.seekSeconds })}
        >
          Forward 10 s
        </button>
        <button type="button" className="rounded bg-dymo-red px-4 py-2 font-mono" onClick={() => run("mpv_stop")}>
          Stop
        </button>
      </section>

      <section aria-label="Picture" className="mb-6">
        <label className="font-mono text-sm">
          CRT preset{" "}
          <select
            className="ml-2 rounded bg-plastic px-3 py-2"
            value={crtPreset}
            onChange={(event) => changeCrtPreset(event.target.value)}
          >
            {config.crtPresets.map((preset) => (
              <option key={preset}>{preset}</option>
            ))}
          </select>
        </label>
      </section>

      <dl className="grid max-w-3xl grid-cols-[10rem_1fr] gap-y-2 rounded bg-black/40 p-4 font-mono text-sm">
        <dt className="text-paper-muted">File</dt>
        <dd>{fileName}</dd>
        <dt className="text-paper-muted">Time</dt>
        <dd className="text-vfd">
          {formatTime(player?.timePos ?? null)} / {formatTime(player?.duration ?? null)}
          {isPlaying && player.paused && " (paused)"}
        </dd>
        <dt className="text-paper-muted">Decoder</dt>
        <dd>{describeHwdec(player?.hwdec ?? null)}</dd>
        <dt className="text-paper-muted">Drive</dt>
        <dd>{volume ? `${volume.volumeName ?? "?"} at ${volume.mountPoint}` : "pick a file"}</dd>
        <dt className="text-paper-muted">Volume UUID</dt>
        <dd className="text-osd">{volume?.volumeUuid ?? "–"}</dd>
      </dl>

      {error && (
        <p role="alert" className="mt-4 max-w-3xl rounded bg-dymo-red/30 p-3 font-mono text-sm">
          {error}
        </p>
      )}
    </main>
  );
}

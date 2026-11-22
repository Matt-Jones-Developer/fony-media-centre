# Fony Media Centre app

Tauri 2 (Rust) with a React + TypeScript + Vite + Tailwind UI. mpv runs as its own process and
the Rust core drives it over mpv's JSON IPC socket.

## Run it

```bash
cd app
pnpm install          # first time only
pnpm tauri dev        # opens the app; Rust and UI both rebuild on save
```

You need Rust (`rustup`), Node, pnpm and mpv (`brew install mpv`).

## Tests

```bash
cd app/src-tauri
cargo test                                   # quick tests, no drive needed
cargo test -- --include-ignored --nocapture  # also plays real files from Glitchy_2TB
```

The drive tests play an mkv, an mp4 and an avi, check seek, pause, shader switching and the
volume UUID, and confirm mpv quits. They only read from the drive.

## Where things are

| Path | What |
| --- | --- |
| `src-tauri/src/mpv.rs` | Starts mpv, sends commands, forwards playback state to the UI |
| `src-tauri/src/volume.rs` | Finds a path's drive and reads its volume UUID |
| `src-tauri/src/lib.rs` | The commands the UI calls, and the CRT presets |
| `shaders/` | CRT shaders for mpv, with credits |
| `src/dev/MpvDevScreen.tsx` | Phase 0c test bench |

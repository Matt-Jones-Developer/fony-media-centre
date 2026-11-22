# Fony Media Centre

A personal media centre for macOS (M1 Pro MacBook Pro, 16 GB) that shows Glitchy's video library as a shelf of VHS tapes in front of a late-80s Trinitron TV and VCR. Playback uses mpv. Later phases add a streamer (P2) and a Raspberry Pi 5 "cable box" (P3).

Personal use only, never distributed.

**Start every session by reading `docs/handoff.md`.** It says where the build is, what was decided last time and the next action.

- Full spec: `docs/spec.md`
- Look and feel: `docs/design-direction.md`
- Mockups: `docs/mockups/` (live canvas: https://claude.ai/artifact/DGM4unNGpV9nqAQaW4tfB5)
- App: `app/README.md` (how to run, test, and where the code lives)
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre (public)

## Read map

| Task | Read |
| --- | --- |
| Starting a session | `docs/handoff.md`, then the current phase below |
| Anything touching the drive | Ground rules, The library on disk |
| UI work | Design essentials, `docs/design-direction.md`, `docs/mockups/` |
| Playback or shaders | `app/README.md`, `app/shaders/README.md` |
| Renaming files | tapehead |

## Ground rules (read first)

1. **The media drives are Glitchy's only archive.**
   - The app must never rename, move, modify or delete media files.
   - The app reads; tapehead (below) is the only thing that renames, and it does so with dry runs, confidence levels and undo logs.
   - Write app state (DB, caches, artwork) to `~/Library/Application Support/FonyMediaCentre/`, never onto the drives.
   - The only exception is the per-show `.tapehead-series.yaml` markers tapehead writes. The app may read them.
2. **Secrets:** `.env.local` holds the TMDB key (`TMDB_API_KEY`). Never commit it, print it or copy it into source. The repo is public.
3. **Git:** don't commit or push unless asked. Commits are authored as Glitchy only, with no attribution lines.
4. **Ask before big dependency choices** that aren't already settled below.

## Stack (decided)

| Area | Choice |
|---|---|
| Shell | **Tauri 2** (Rust core, WKWebView on macOS). Glitchy is new to Rust: keep the Rust small, clear and commented, and explain it in plain English |
| UI | **React + TypeScript + Vite**, Tailwind v4 (tokens in `app/src/index.css`); react-three-fiber only where 3D genuinely helps (tape pull-out/flip) |
| Package manager | **pnpm** |
| Playback | **mpv**, controlled over its JSON IPC socket (`--input-ipc-server`). Fullscreen mpv window for the MVP. Dev uses Homebrew mpv; the packaged app bundles an mpv build with **libmpv**, needed for previews playing inside the on-screen TV |
| CRT effect | mpv GLSL shaders during playback (`app/shaders/crt-lottes.glsl`, presets Trinitron, Portable, Arcade, Clean); CSS/WebGL on the UI |
| Data | SQLite (via `rusqlite` or `sqlx`, your call; explain the choice) |
| Media tools | ffprobe/ffmpeg (Homebrew, installed) |
| Metadata | TMDB API, key from `.env.local` (`TMDB_API_KEY`), read only in Rust |
| Renamer | `tools/tapehead` (Python, uv). Already built and in use: don't rewrite it |

Why not Next.js: Tauri serves a static bundle with no Node server, so Vite SPA + Rust core is the right split.

## Repo layout

```
fony-media-centre/
  CLAUDE.md              ← you are here
  docs/                  handoff, spec, design direction, mockups
  tools/tapehead/        Python renamer CLI (survey → match → plan → apply → clean)
  app/                   Tauri 2 + React app (see app/README.md)
  .scratch/              local scratch + backups (gitignored; contains rename backups, never delete)
```

## The library on disk (what the scanner must understand)

Drive: `/Volumes/Glitchy_2TB/MEDIA/` (volume UUID `D2946B44-EEF5-3A81-A7BE-648CD3312449`). Cartoons have been tidied by tapehead:

```
Cartoons/
  Show (Year)/
    .tapehead-series.yaml              # name, year, mode, tmdb_id, tmdb_name
    Season 01/Show (Year) - S01E03 - Title.mkv
    Season 01/Show (Year) - S01E03-E04 - Title A & Title B.mp4   # double/range
    Season 01/Show (Year) - S01E01a - Segment Title.mkv          # half-episode segment a/b
    Specials/Show (Year) - S00E02 - Title.mkv
    Movies/Title (Year).mp4            # film tied to the series → its own tape, stacked with the series
    Extras/Featurette.mkv              # appended after the last season
```

Rules:
- Identify drives by **volume UUID**, not name. Titles on an unplugged drive stay on the shelf, greyed out as "tape not in shelf".
- **No DVD folders.** All VIDEO_TS rips were converted to mkv. Don't build VIDEO_TS support.
- `Cartoons/.tapehead/series.yaml`:
  - `mode: skip` entries are **hidden** (e.g. the main Pink Panther Cartoon Collection).
  - `mode: keep` entries are shown **as-is** under their `name`/`year` (e.g. the "BEST of the Pink Panther" folder, shown as The Pink Panther (1964)). Read it for those exclusions until the app has its own editor.
- **Folders without a `.tapehead-series.yaml` marker** (as of 2026-11-22):
  - `Dungeons & Dragons (1983)` and `Little Miss (1983)`: already in the tidy layout and TMDB-named, made by hand rather than by tapehead.
  - `He-Man and the Masters of the Universe S01-S02 (1983-)` and `Looney Tunes - Golden Collection`: new, not tidied yet.
  - `The Pink Panther Cartoon Collection`: hidden or kept via `series.yaml`, as above.
  - The scanner still parses unmarked folders (reuse tapehead's parsing rules as the reference: `tools/tapehead/src/tapehead/parse.py`) and flags untidy ones for the editor's review queue rather than hiding them.
- Other folders (Movies, etc.) haven't been tidied yet.

## Build plan

Work phase by phase. **Finish each phase's acceptance checks, then stop and summarise for Glitchy before starting the next.** Update `docs/handoff.md` at the end of every session.

| Phase | Status | What |
| --- | --- | --- |
| 0a | ✅ done | tapehead built; Cartoons tidied |
| 0b | ✅ done | Mockups in `docs/mockups/` |
| 0c | ✅ done 2026-11-22 | Tauri scaffold, mpv IPC, CRT presets, volume UUID |
| **1** | **▶ next** | MVP Mac app (below) |
| 2 | later | Streamer and import |
| 3 | later | Pi 5 cable box |

### Phase 0c: scaffold + mpv spike ✅
Done 2026-11-22. What exists and how to run it: `app/README.md`. Acceptance results: `docs/handoff.md`.

### Phase 1: MVP (Mac app)
1. **Library core:**
   - scanner (the rules above)
   - ffprobe metadata
   - SQLite schema: drives, series, seasons, episodes, movies, files, tapes, user overrides
   - FSEvents drive watching, offline drives
2. **Metadata & art:**
   - TMDB lookups, using the `tmdb_id` in markers where present
   - art cache in Application Support
   - sleeve/spine generator (decided, see `docs/spec.md` §5):
     - front case: the TMDB poster
     - spine: TMDB's title logo on a colour taken from the poster, season number underneath
     - wall posters: show poster left, season poster right
3. **Home screen:** match `docs/mockups/Main.dc.html` (the realistic FONY look, not the cartoony one):
   - FONY Trinitone TV on a black-ash unit, FONY VCR in the cubby
   - display on the left; cassette flap and transport panel on the right
   - two 2:3 poster areas on the wall, filled from TMDB per selected tape
   - a Dymo-tab top shelf (Saturday Morning / Continue / Just Added / Favourites)
   - the power-on screen from `Boot.dc.html`
   - don't use the transparent `vhs-*.png` tape props yet
4. **Shelf view:**
   - endlessly scrolling shelves, spine-out tapes, series movies stacked beside their seasons
   - pull out → flip → Insert
   - filters (`Shelf.dc.html` is layout only; restyle it to the realistic look)
5. **Player:** mpv with a VCR on-screen display, resume position, Be Kind Rewind prompt, audio track picker (no subtitles needed), CRT presets.
   - On launch, close any mpv left behind by a crashed session (its socket is `fony-mpv-<pid>.sock` in the temp folder).
6. **Library editor & import settings:**
   - **Per tape:** rename, fix the TMDB match, swap art (front, spine, back, label text), tape style, which shelf it sits on, merge/split, hide.
   - **Per drive/folder:** include/exclude paths.
   - **Review queue** for un-tidied/new files.
   - **Poster and sticker drop-ins** (Glitchy's own images).
   - The editor changes the app's DB only, never files on the drive.
7. **Packaging:** signed .app with mpv (libmpv build) bundled.
8. **Later in Phase 1 or straight after:** previews playing inside the on-screen TV via libmpv's render API.

### Phase 2: streamer & import (later)
- Serve the library to other devices (ffmpeg HLS).
- Internet Archive / free-API browser that imports into the library.

### Phase 3: Pi 5 cable box (later, separate build)
- Rule-based channels with a deterministic "live" schedule (`(now − epoch) mod playlist length`).
- Watch-from-start, favourites, TV-guide channel.
- Kiosk boot, Bluetooth remote.
- WebKitGTK performance must be benchmarked early.

## Design essentials (details in `docs/design-direction.md`)
- **Realistic, not cartoony:** real materials, grain, gloss, shadows. No vaporwave or neon.
- **Hardware is FONY-branded** (parody). Don't recreate real Sony marks; Glitchy may drop in his own photos.
- **Fonts:**
  - VT323 for OSD/VFD
  - Alfa Slab One (scaled X 1.18) for the FONY wordmark
  - Inter for small labels
  - Kalam for handwritten tape labels
  - Roboto Mono bold for Dymo labels
- **Colours:** Tailwind tokens in `app/src/index.css` (`ash`, `plastic`, `vfd`, `osd`, `av-blue`, `dymo-*`, …). Use the tokens, never raw hex in components.
- **Accessibility:** tapes and controls are real `<button>`s with labels, and everything works by keyboard (P3 is remote-driven, so build arrow-key navigation from day one).

## tapehead (the renamer)
```bash
cd tools/tapehead && uv run pytest                 # 37 tests
uv run tapehead run "/Volumes/Glitchy_2TB/MEDIA/Cartoons"                   # report only
uv run tapehead run "/Volumes/Glitchy_2TB/MEDIA/Cartoons" --apply --clean   # tidy what it's sure of
```
Only touch tapehead when asked. Its rule: **never rename something wrongly or make a name worse than it was; leave it as-is when unsure.**

If a tapehead plan would move files into slots that are already filled (old numbering that disagrees with TMDB), don't apply it. Match by title instead and hand-build a plan in tapehead's JSON format, so `apply` and `undo` still work. That's how the 2026-11-22 straggler run was done.

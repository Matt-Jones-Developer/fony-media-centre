# Fony Media Centre — Spec & Build Plan (v3)

_Last updated: 22 Nov 2026_

## 1. Vision
A personal media centre for an M1 Pro MacBook Pro (16GB, macOS, Homebrew + ffmpeg installed) with a late-80s VCR/CRT look. The library is shown as physical VHS tapes on shelves, with spine and front-cover artwork. Media lives on several external SSDs. A separate Raspberry Pi 5 build later becomes a retro "cable box" with live channels. Look and feel: see **design-direction.md**. It's personal use only and won't be distributed.

## 2. Decisions so far
| Area | Decision |
|---|---|
| App shell | **Tauri 2** (Rust core, system webview). Glitchy new to Rust — keep core small, learn as we go |
| UI | **React + TypeScript + Vite**, Tailwind, react-three-fiber for 3D bits |
| Playback | **mpv** (same engine as IINA), controlled over JSON IPC. Fullscreen playback for MVP |
| CRT effects | mpv GLSL shaders during playback; WebGL/CSS on the UI. Presets + "clean" mode |
| DB | SQLite |
| Media tools | ffprobe/ffmpeg |
| Metadata | TMDB API (films, TV, episode titles) |
| Renamer | Standalone **Python CLI `tapehead`**, TMDB-backed (built and applied to Cartoons, see §4) |
| Subtitles | Not needed. The renamer's clean step deletes them |
| P2 sources | Internet Archive + free/open APIs, import into library |
| P3 | Separate Pi 5 build; SSDs move to the Pi |

### Why React + Vite rather than Next.js
Tauri serves a static bundle from inside the app; there's no Node server at runtime. Next.js's main strengths (SSR, server components, API routes, image optimisation) all need that server. Vite gives a plain SPA with instant HMR, and the Rust core plays the backend role.

### Tauri vs Electron
- **Mac:** no meaningful visual difference. Tauri (WKWebView) is far lighter (~10MB app vs ~150MB) and starts faster. Video is mpv either way.
- **Pi:** Electron has the edge (WebKitGTK's ARM WebGL lags Chromium's). Mitigations:
  1. Lighter 2D 10-foot UI on the Pi.
  2. The core is a self-contained service, so a Chromium kiosk UI is possible.
  3. Benchmark early in P3.

## 3. Library conventions
```
/Volumes/<SSD>/MEDIA/
  Movies/Title (Year)/Title (Year).mkv
  Cartoons/Show (Year)/Season 01/Show (Year) - S01E03 - Episode Title.mkv
  Cartoons/Show (Year)/Specials/...            # S00 / pilots / stray segments
  Cartoons/Show (Year)/Movies/Title (Year).mp4 # films tied to a series
  Cartoons/Show (Year)/Extras/...              # featurettes, pencil tests
```
- Each SSD is identified by its volume UUID, not its name.
- Titles on unplugged drives stay on the shelf, greyed out as "tape not in shelf".
- Optional per-title override art: `poster.jpg`, `spine.png`, `back.jpg` in the title folder.
- Episode codes can carry a segment letter (`S01E01a`) for half-episodes, or a range (`S01E03-E04`) for two-story files. The scanner must understand both.
- Tidied show folders carry a `.tapehead-series.yaml` marker (name, year, mode, tmdb_id, tmdb_name).

### How the shelf treats each kind
- **Series:** one VHS per season, or box-set style for long runs.
- **Series movies** (`Movies/` inside a show): their own VHS, stacked next to that show's seasons (e.g. the G1 movie beside the four seasons).
- **No DVD rips:** VIDEO_TS folders were converted to mkv (2026-11-22). No VIDEO_TS support is needed.
- **Extras:** tacked onto the end of the last season's tape, as bonus items after the final episode.
- **"Custom" series:** shows with odd numbering use their filename numbering rather than TMDB, but still get a proper VHS. Currently Dexter's Laboratory, and Pinky and the Brain (whose Animaniacs segments sit in its Specials).
- **Exclusions:** some folders stay on the drive but are hidden from the app.
  - Currently: the main Pink Panther Cartoon Collection.
  - Its `The BEST of the Pink Panther Collection` subfolder is Glitchy's curated set. It is shown as **The Pink Panther (1964)**, and its files are never renamed.
  - Read from tapehead's `series.yaml` (`mode: skip` = hidden, `mode: keep` = shown as-is) until the app's own editor exists.
- **Un-tidied folders** (new arrivals such as He-Man and Looney Tunes): parsed and shown, flagged in the editor's review queue. The 2026-11-22 straggler run filed the earlier leftovers.

## 4. Tool: `tapehead` (built, Phase 0a ✅)
Lives in `tools/tapehead`.

**Workflow:** `survey → match → plan → apply → clean`, or `run` for all of them in one go.
- `survey` is read-only and writes `.tapehead/series.yaml` (editable; edits survive re-runs).
- `match` links each series to TMDB.
- `plan` is a dry run that writes JSON and CSV.
- `apply` moves files and writes a portable undo log (relative paths).
- `undo` reverses an apply.
- `clean` deletes torrent junk, subtitles, sample clips and folders left with no video. It's a dry run unless given `--really`.

**Series modes:**
- `tmdb`: proper titles.
- `local`: filename numbering ("custom").
- `keep`: show as-is, don't rename.
- `skip`: leave alone and hide.
- Plus `force_season` (e.g. 0 = Specials).

**Confidence:** high / medium / low. Only high is applied by default. Titles beat numbers, which fixes DVD vs aired order. Number-only matches are trusted only when the numbering is verified. Problem files stay as-is.

**Cartoons run (13 Nov 2026):**
- 1,894 episodes, 4 movies and 2 extras tidied.
- 262 junk files and 48 empty folders removed.
- Backups are in `.scratch/backups/`.

## 5. UI concept
- **Boot:** the CRT powers on (degauss thunk), then a dark-blue power-on screen: FONY / MEDIA CENTRE, green AV1, blinking INSERT TAPE.
- **Home screen:**
  - A FONY Trinitone TV on a black-ash unit, with the FONY VCR in the cubby below. Display on the left; cassette flap and transport panel on the right.
  - Two 2:3 poster areas on the wall.
  - A Dymo-tab top shelf: Saturday Morning / Continue / Just Added / Favourites.
  - Hovering over a tape plays a short ffmpeg-generated preview clip on the CRT (later).
- **Shelf view:** an endlessly scrolling wall of shelves.
  - Spine-out tapes, with some face-out.
  - Sort and filter by genre, decade, A–Z, Movies/Cartoons.
  - Selecting a tape: it slides out in 3D, flips to the back cover, then Insert.
- **Sleeve generator:**
  - Cartoons: home-recorded E-180s with handwritten labels.
  - Movies: rental clamshells.
  - Built from TMDB art.
  - **Front case:** the show's TMDB poster.
  - **Spine** (decided 2026-11-22): black tape spine with a label strip carrying TMDB's transparent title logo on a colour taken from the poster, season number underneath. No API serves real VHS spine scans; those come later as editor drop-ins.
- **Wall posters** (decided 2026-11-22): when a tape is selected, the left poster shows the show's main TMDB poster and the right shows that season's poster (a second show poster if TMDB has none).
- **Player:** mpv fullscreen with a VCR on-screen display, tracking noise on seek, resume, and a Be Kind Rewind prompt. CRT presets: Trinitron / Portable / Arcade / Clean.
- **Library editor & import settings (MVP):**
  - **Per title:** rename, fix the TMDB match, swap or upload art (front, spine, back, label text and handwriting), change tape style (home-recorded / rental / retail), pick which shelf it sits on, merge or split tapes, hide or exclude.
  - **Per drive/folder:** include/exclude paths, default mode (TV/cartoon/movie).
  - **Re-scan & import:** a review queue for new or unmatched files, showing the same high/medium/low confidence as tapehead.
  - **Decor:** posters and a sticker drawer where Glitchy can drop in his own images.
  - The editor changes the app's database only, never media files.
- **Later:** video inside the CRT on the home screen (libmpv render API).

## 6. Architecture
```
React UI (shelf · sleeves · VCR · OSD · editor)
   ⇅ Tauri commands/events
Rust core: drive registry · scanner · ffprobe · metadata/art cache · mpv controller · (P3 scheduler)
   ↓                                  ↓
SQLite library DB             mpv (bundled) + CRT GLSL shaders
(~/Library/Application Support/FonyMediaCentre/)
```

## 7. Phases
**Phase 0 — Foundations**
- 0a ✅ `tapehead` built; Cartoons tidied.
- 0b ✅ Home-screen mockups (`docs/mockups/`, live canvas linked in CLAUDE.md).
- 0c ✅ Tauri + React scaffold and mpv IPC spike (2026-11-22):
  - Play mp4/mkv/avi from the SSD with VideoToolbox hardware decoding.
  - Switch CRT shaders.
  - Read the volume UUID.

**Phase 1 — MVP (Mac app)**
1. Library core: scanner (understands segments, ranges, Movies/, Extras/, markers, exclusions), ffprobe, SQLite, FSEvents drive watching, offline drives.
2. Metadata and art: TMDB, art cache, sleeve generator.
3. Home screen: Fony TV + VCR + top shelf, boot sequence.
4. Shelf view: endless shelves, 3D pull-out and flip, filters, series + movie stacking.
5. Player: mpv control, VCR OSD, resume, CRT presets.
6. Library editor and import settings (§5).
7. Packaging: signed .app with mpv bundled.

**Phase 2 — Streamer & import**
- Serve the library to other devices (ffmpeg HLS).
- Internet Archive browser + free APIs, importing into the library.

**Phase 3 — Cable box (Pi 5 build)**
- Rule-based channels and a deterministic "live" schedule.
- TV-guide channel, favourites, watch-from-start.
- Kiosk boot, Bluetooth remote.
- The Pi 5 hardware-decodes HEVC only; the rest decodes on the CPU (fine at SD/1080p).

## 8. Open items
- Survey the other SSD folders (Movies etc.).
- Glitchy may supply FONY-edited photos of the TV/VCR to replace the CSS hardware.

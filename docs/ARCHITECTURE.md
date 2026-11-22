# Architecture

How the app fits together. One-way decisions and their reasons are in `docs/DECISIONS.md`.

## Library core (Phase 1, step 1)

Status: **approved 2026-11-22.** Slice 1 built.

### 1. Requirements

What it does:

- Finds the media drives that are plugged in, by volume UUID.
- Reads `Cartoons/` into a library: shows, seasons, episodes (with segments `a`/`b` and ranges `E03-E04`), specials, series movies and extras.
- Uses the tapehead markers (`.tapehead-series.yaml`) and `Cartoons/.tapehead/series.yaml` (`skip` hides, `keep` shows as-is).
- Parses folders with no marker too, and flags anything it can't place for the review queue.
- Reads duration and video size with ffprobe, in the background.
- Notices new, renamed or removed files while the app is open, and drives being plugged in or out.
- Keeps an unplugged drive's titles on the shelf, greyed out.

Limits:

- **Never writes to a drive.** The database lives in `~/Library/Application Support/FonyMediaCentre/library.db`.
- Opening the app shows the shelf from the database straight away. A rescan runs behind it.
- A rescan of Cartoons (without ffprobe) finishes in under 2 seconds.
- One user, one Mac, offline. No accounts, no server.

### 2. Size

Measured on Glitchy_2TB today:

| Folder | Video files |
| --- | --- |
| Cartoons | 2,793 (30 show folders) |
| Shows | 674 |
| Anime | 345 |
| Movies | 144 |
| Everything else | about 60 |

- Walking all of Cartoons takes about 15 ms once the drive's folder list is cached.
- One ffprobe call takes about 70 ms. 2,800 files is about 3.5 minutes one at a time, under a minute with 4 at once. It only runs for new or changed files, so after the first scan it is near zero.
- Even at 10x (40,000 files) the database is a few megabytes. SQLite handles that without trying. **The hard part is getting the parsing right, not speed.**

### 3. Parts

| Part (Rust module) | Its one job | Owns | Calls |
| --- | --- | --- | --- |
| `library/db.rs` | Opens the database, runs migrations, does every write | All library tables | SQLite |
| `library/parse.rs` | Turns a file path into show, season, episode, segment, title. Plain functions, no file access | Nothing | Nothing |
| `library/markers.rs` | Reads the tapehead YAML files (with `serde_norway`) | Nothing | The drive (read only) |
| `library/scan.rs` | Walks a folder, works out what changed since last time, hands the changes to `db` | Nothing | `parse`, `markers`, `db` |
| `library/probe.rs` | Runs ffprobe on files that need it, a few at a time | Nothing | ffprobe, `db` |
| `drives.rs` (grows from `volume.rs`) | Lists mounted drives and their UUIDs | Nothing | `diskutil` |
| `watch.rs` | Tells `scan` when files or mounted drives change | Nothing | macOS file events |
| `lib.rs` | The commands the UI calls | Nothing | `db` (reads), `scan` |

```mermaid
flowchart TD
  UI[React UI] -->|Tauri commands| Cmd[lib.rs commands]
  Cmd --> Scan[scan.rs]
  Cmd -->|reads| DB[db.rs]
  Watch[watch.rs] --> Scan
  Scan --> Parse[parse.rs]
  Scan --> Markers[markers.rs]
  Scan --> Drives[drives.rs]
  Scan -->|writes| DB
  Probe[probe.rs] -->|writes| DB
  DB --> SQLite[(library.db)]
  Markers -.read only.-> Drive[(Media drive)]
  Probe -.read only.-> Drive
```

Only `db.rs` touches SQLite. Only `scan.rs` and `probe.rs` read the drive. The UI never sees a path it can open itself; it asks Rust to play a file by id.

### 4. Main journeys

**App opens**

1. UI asks for the library. `db` returns what was there last time, with each drive marked online or offline. (instant)
2. In the background, `drives` lists what's mounted and matches UUIDs to known drives.
3. For each online drive, `scan` walks each library folder, compares with the `files` table (path, size, modified time) and writes only the differences in one transaction.
4. Rust sends a `library-changed` event. UI refetches.
5. `probe` works through files with no ffprobe data, writing results in small batches, sending `library-changed` every batch.

**A file changes while the app is open** (tapehead renames, a new show is copied in)

1. `watch` gets file events for that folder and waits 2 seconds for things to settle.
2. `scan` rescans that whole library folder. It's fast enough that working out which show changed isn't worth the code.
3. Same as steps 3 to 5 above.

**A drive is unplugged**

1. `watch` sees the volume leave `/Volumes`.
2. The drive is marked offline. Nothing is deleted. The UI greys its tapes out.
3. When it comes back, it's rescanned.

### 5. Commands (UI to Rust)

All return `Result<T, String>` like the existing mpv commands, with a plain error message.

| Command | Does | Returns |
| --- | --- | --- |
| `library_overview` | Everything the shelf needs: series, seasons, movies, with drive online state | A list, small enough to send whole (30 shows) |
| `library_series(id)` | One show's episodes, specials and extras in play order | A list |
| `library_rescan` | Starts a rescan now | Nothing; progress comes as events |
| `library_review_queue` | Files the scanner couldn't place, and untidy folders | A list |

Playing a file moves from `mpv_load(path)` to `play_file(file_id)`: Rust looks up the path, so the UI never handles raw paths.

### 6. Database

SQLite, WAL mode, foreign keys on. Ids are plain integers (local only). Times are UTC ISO strings. Paths are stored **relative to the drive**, so a drive mounted under a different name still matches.

```mermaid
erDiagram
  drives ||--o{ library_roots : has
  drives ||--o{ files : has
  library_roots ||--o{ series : contains
  series ||--o{ seasons : has
  seasons ||--o{ episodes : has
  series ||--o{ movies : has
  series ||--o{ extras : has
  files ||--o| episodes : "is"
  files ||--o| movies : "is"
  files ||--o| extras : "is"
```

| Table | Key columns | Notes |
| --- | --- | --- |
| `drives` | `volume_uuid` UNIQUE, `name`, `last_mount_point`, `last_seen_at` | Online or offline is worked out at runtime, not stored |
| `library_roots` | `drive_id`, `rel_path` (e.g. `Cartoons`), `kind` (cartoons, movies, shows), `enabled` | UNIQUE (`drive_id`, `rel_path`). The editor's include/exclude later. Seeded with Cartoons only |
| `series` | `root_id`, `folder` UNIQUE per root, `name`, `year`, `tmdb_id`, `mode` (tmdb, local, keep), `source` (marker, series_yaml, parsed), `is_tidy` | `skip` folders are not stored at all |
| `seasons` | `series_id`, `number` (0 = Specials) | UNIQUE (`series_id`, `number`) |
| `episodes` | `season_id`, `file_id` UNIQUE, `episode`, `episode_end` (null unless a range), `segment` (null, a, b), `title` | Index on (`season_id`, `episode`, `segment`) for play order |
| `movies` | `series_id`, `file_id` UNIQUE, `title`, `year` | `series_id` nullable later for standalone films (Movies folder) |
| `extras` | `series_id`, `file_id` UNIQUE, `title` | Played after the last season |
| `files` | `drive_id`, `rel_path`, `size`, `modified_at`, `duration_ms`, `video_codec`, `width`, `height`, `probed_at` | UNIQUE (`drive_id`, `rel_path`). Index on `probed_at` for the probe queue |

Not in this step:

- **Tapes, user overrides, watch progress.** They come with the shelf (step 4), the editor (step 6) and the player (step 5), as new migrations. Today a tape is simply "one season" or "one movie", worked out on read.
- **Audio tracks.** mpv reports them when a file plays, so storing them buys nothing yet.
- **The review queue** is a query (files with no episode, movie or extra; series with `is_tidy = 0`), not a table.

Removing rows: a file that's gone from an **online** drive is deleted, along with its episode row. Files on an **offline** drive are never touched.

Migrations: numbered `.sql` files built into the app, tracked with SQLite's `user_version`. No migration library.

### 7. Caching

None. The database is the cache of the drive, and every query is tiny.

### 8. When things go wrong

| Problem | What happens | What Glitchy sees |
| --- | --- | --- |
| Drive unplugged mid-scan | Scan stops, writes nothing for that drive | Tapes grey out |
| ffprobe missing or fails on a file | That file is skipped and retried next launch | Tape shows no running time |
| A file name can't be parsed | File goes to the review queue | Listed in the editor (later), counted on the dev screen now |
| Bad YAML in a marker | Folder treated as unmarked, parsed from file names | Flagged in the review queue |
| Database file corrupt | App starts with an empty library and rebuilds it by rescanning (nothing in it can't be rebuilt yet) | A one-line notice |
| tapehead renames files | The old paths disappear and new ones appear on the next scan | Nothing; the show looks the same |

Logs record what failed and which step, never the full path list.

### 9. When it would need to change

- **The other folders** (Movies, Shows, Anime) aren't tidy. Adding them is a new `library_roots` row plus parse rules for films, not a redesign.
- **Watch progress keyed by file** would be lost when tapehead renames a file. Decide in the player step whether to key it by show, season and episode instead (see DECISIONS).
- **Phase 3 (Pi)** reads the same schema. Nothing here is macOS-only except `diskutil` and file events, both behind one module each.

### 10. Build order

Each is one `plan-ticket` slice and ends with something Glitchy can check.

1. **Database and drives.** Create the database, run migration 1, record Glitchy_2TB, report online or offline. Check: the dev screen shows the drive.
2. **Parser.** Port tapehead's rules for the tidy layout (`SxxEyy`, segments, ranges, Specials, Movies, Extras) plus marker reading. Rust tests copied from tapehead's cases. Check: `cargo test`.
3. **Scanner.** Walk Cartoons into the database, with the rescan command and a count on the dev screen. Check: 30 shows minus hidden ones, about 2,800 files placed, the rest listed.
4. **ffprobe.** Background probing. Check: running times filled in, app stays responsive.
5. **Watching.** File changes and plug/unplug. Check: rename a test copy or unplug the drive and watch the dev screen update.
6. **Unmarked folders.** Parse He-Man and Looney Tunes as far as tapehead's loose rules allow; the rest to the review queue.

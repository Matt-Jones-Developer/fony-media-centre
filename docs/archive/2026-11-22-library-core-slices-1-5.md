# Library core, slices 1 to 5 (2026-11-22)

Design and decisions: `docs/ARCHITECTURE.md` (Library core) and `docs/DECISIONS.md`.

| Slice | What | Where | Result |
| --- | --- | --- | --- |
| 1. Database and drives | SQLite in Application Support, all tables in migration 1, drives found by volume UUID | `library/db/mod.rs`, `drives.rs` | Checked by Glitchy |
| 2. Parser and markers | Tidy layout rules, tapehead YAML (`serde_norway`), `series.yaml` folder rules | `library/parse.rs`, `library/markers.rs` | All 2,066 files in tidied folders placed |
| 3. Scanner | Walks Cartoons, saves in one transaction, ids stable across scans | `library/scan.rs`, `library/db/scan_store.rs` | 30 shows (3 untidy), 2,103 episodes, 4 movies, 2 extras, 562 unplaced. Scan takes about 0.1 s |
| 4. ffprobe | Background, 4 at a time, batches of 50, 30 s cap per file | `library/probe.rs`, `library/db/probe_store.rs` | 2,671 files in 37 s, none failed, about 800 hours |
| 5. Watching | `notify` on `/Volumes` and each online library folder, 2 s quiet before rescanning, 60 s cap | `watch.rs` | File events proven on a temp folder. Plug and unplug checked by Glitchy |
| 6. Untidy folders | Looser rules for unmarked and kept folders, tried after the tidy ones: loose `S01E01`, numbered `004 Title (date)`, dated `MMDDYY` shorts grouped by decade, Extras at any depth, folder name cleanup | `library/loose.rs`, `library/scan.rs` | 2,669 placed, 2 unplaced (Private Snafu). He-Man, Looney Tunes and BEST of Pink Panther still flagged untidy |

Dev screen: `app/src/dev/LibraryPanel.tsx` shows drives, counts, probe progress and the unplaced list, with **Rescan** and **Add library folder**.

Settled with Glitchy: `rusqlite`; Cartoons only in Phase 1; unmarked folders go to the review queue; `notify`, `serde_norway` and `regex` approved.

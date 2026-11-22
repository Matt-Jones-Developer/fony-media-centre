# tapehead

Tidies messy TV and cartoon filenames into a clean, consistent library for Movie Centre:

```
Cartoons/
  Count Duckula (1988)/
    Season 01/
      Count Duckula (1988) - S01E02 - Vampire Vacation.mp4
  The Ren & Stimpy Show (1991)/
    Season 01/
      The Ren & Stimpy Show (1991) - S01E01a - Stimpy's Big Day.mkv
```

**Safety rules:**
- Nothing moves until you run `apply` on a plan you've reviewed.
- Every move is logged, and `undo` reverses it.
- Moves happen on the same drive, so they're instant and nothing gets copied.
- Nothing is ever deleted.

## Setup (once)

```bash
cd ~/builds/media-centre/tools/tapehead
uv sync
uv run tapehead config --tmdb-key YOUR_KEY   # free from themoviedb.org → Settings → API
```

To get a `tapehead` command available anywhere: `uv tool install --editable .`

## The workflow

```bash
tapehead survey "/Volumes/Glitchy_2TB/MEDIA/Cartoons"   # read-only overview → .tapehead/series.yaml
tapehead match  "/Volumes/Glitchy_2TB/MEDIA/Cartoons"   # link each series to TMDB (asks when unsure)
tapehead plan   "/Volumes/Glitchy_2TB/MEDIA/Cartoons"   # dry run → plan-*.json + plan-*.csv
tapehead apply  "/Volumes/.../Cartoons/.tapehead/plan-YYYYMMDD-HHMMSS.json"
tapehead undo   "/Volumes/.../Cartoons/.tapehead/undo-YYYYMMDD-HHMMSS.jsonl"
```

### Adding more later

Drop new show folders into the library folder, however messy they are, then:

```bash
tapehead run "/Volumes/Glitchy_2TB/MEDIA/Cartoons"                   # report only
tapehead run "/Volumes/Glitchy_2TB/MEDIA/Cartoons" --apply --clean   # tidy what it's sure about
```

`run` does survey → auto-match → plan, and with the flags, apply (high confidence only) → clean.

- Tidied shows carry a hidden `.tapehead-series.yaml`, so they're recognised and left alone.
- New shows it can't confidently match are listed. Run `tapehead match` to sort those out.
- TMDB episode lists are saved in `.tapehead/tmdb/`, so a plan can be re-made without the internet.

### series.yaml

`survey` writes one entry per series it finds. You can edit it, and re-running `survey` keeps your edits.

- `mode: tmdb` looks up proper episode titles. `mode: local` keeps the numbers and titles already in the filenames (good for things TMDB doesn't list well, like the Pink Panther theatrical shorts). `mode: skip` leaves a series alone.
- `tmdb_id` is the number in a TMDB URL (`themoviedb.org/tv/2504` → `2504`).
- `name` and `year` decide the folder name.

### Confidence levels

| Level | Meaning | Applied by default? |
|---|---|---|
| high | Number and title agree, or a strong title match, or number only when there was no title to check | ✅ |
| medium | Number matched but the title didn't confirm it (or vice versa) | `--include medium` |
| low | Something's off, e.g. the files are numbered per segment but TMDB numbers per episode | `--include low` |

**Title matches beat numbers.** If a file says S01E05 but its title clearly matches TMDB's S01E07, the file gets renumbered to S01E07. That fixes DVD-order vs aired-order mix-ups, which are common with cartoons.

## What it understands

`S01E02` · `S1E2` · `S01 E02` · `S01.E02` · `S1-E1` · `S02E024` · `S03E16 E17` (double episode) · `1x02` · `1x03a` (half episode) · `1e01a` · `201` inside a Season 2 folder · `Ep. 01` · `E97` · `MASK - 39 Title` · `ulysses.31.01.title` · `001 Title (Dec 18, 1964)` · season folders like `Season 1`, `S03 (360p)`, `Series1` and `1` · subtitles next to the video or in a `Subs/` folder.

It leaves alone (and lists for you): movies inside show folders, DVD `VIDEO_TS` folders, extras and featurettes, sample clips and torrent junk.

## Tests

```bash
uv run pytest
```

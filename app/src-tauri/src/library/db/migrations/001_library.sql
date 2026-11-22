-- The library: what's on the media drives, as last scanned.
-- Paths are relative to the drive, so a drive mounted under a new name still matches.
-- Times are UTC, written like 2026-11-22T14:05:00Z.

CREATE TABLE drives (
  id INTEGER PRIMARY KEY,
  volume_uuid TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  last_mount_point TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- A folder on a drive that the library reads, e.g. MEDIA/Cartoons.
CREATE TABLE library_roots (
  id INTEGER PRIMARY KEY,
  drive_id INTEGER NOT NULL REFERENCES drives (id) ON DELETE CASCADE,
  rel_path TEXT NOT NULL CHECK (rel_path <> ''),
  kind TEXT NOT NULL CHECK (kind IN ('cartoons', 'movies', 'shows')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (drive_id, rel_path)
);

CREATE TABLE files (
  id INTEGER PRIMARY KEY,
  drive_id INTEGER NOT NULL REFERENCES drives (id) ON DELETE CASCADE,
  rel_path TEXT NOT NULL,
  size INTEGER NOT NULL CHECK (size >= 0),
  modified_at TEXT NOT NULL,
  -- The ffprobe columns stay null until the file has been probed.
  duration_ms INTEGER CHECK (duration_ms >= 0),
  video_codec TEXT,
  width INTEGER CHECK (width > 0),
  height INTEGER CHECK (height > 0),
  probed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (drive_id, rel_path)
);
CREATE INDEX files_probed_at ON files (probed_at);

-- One show folder, e.g. "DuckTales (1987)". Folders marked `skip` are never stored.
CREATE TABLE series (
  id INTEGER PRIMARY KEY,
  root_id INTEGER NOT NULL REFERENCES library_roots (id) ON DELETE CASCADE,
  folder TEXT NOT NULL,
  name TEXT NOT NULL,
  year INTEGER,
  tmdb_id INTEGER,
  mode TEXT NOT NULL CHECK (mode IN ('tmdb', 'local', 'keep')),
  source TEXT NOT NULL CHECK (source IN ('marker', 'series_yaml', 'parsed')),
  is_tidy INTEGER NOT NULL CHECK (is_tidy IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (root_id, folder)
);

-- Season 0 is Specials.
CREATE TABLE seasons (
  id INTEGER PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
  number INTEGER NOT NULL CHECK (number >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (series_id, number)
);

-- episode_end is only set for a two-story file (S01E03-E04); segment is a or b for half-episodes.
CREATE TABLE episodes (
  id INTEGER PRIMARY KEY,
  season_id INTEGER NOT NULL REFERENCES seasons (id) ON DELETE CASCADE,
  file_id INTEGER NOT NULL UNIQUE REFERENCES files (id) ON DELETE CASCADE,
  episode INTEGER NOT NULL CHECK (episode >= 0),
  episode_end INTEGER CHECK (episode_end IS NULL OR episode_end > episode),
  segment TEXT CHECK (segment IS NULL OR segment IN ('a', 'b')),
  title TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX episodes_play_order ON episodes (season_id, episode, segment);

-- series_id is null for a film that isn't tied to a show (the Movies folder, later).
CREATE TABLE movies (
  id INTEGER PRIMARY KEY,
  series_id INTEGER REFERENCES series (id) ON DELETE CASCADE,
  file_id INTEGER NOT NULL UNIQUE REFERENCES files (id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  year INTEGER,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX movies_series_id ON movies (series_id);

CREATE TABLE extras (
  id INTEGER PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
  file_id INTEGER NOT NULL UNIQUE REFERENCES files (id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX extras_series_id ON extras (series_id);

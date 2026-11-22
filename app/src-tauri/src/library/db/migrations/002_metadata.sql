-- TMDB metadata for the tapes. Image paths are TMDB's (e.g. /abc.jpg); the files live in the art folder.

-- What TMDB said about one show.
CREATE TABLE tmdb_shows (
  tmdb_id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  first_air_year INTEGER,
  overview TEXT,
  poster_path TEXT,
  logo_path TEXT,
  -- Picked from the poster once it has downloaded, e.g. #8a2b1f.
  spine_colour TEXT,
  fetched_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Joined to our seasons by number. 0 is Specials.
CREATE TABLE tmdb_seasons (
  tmdb_id INTEGER NOT NULL REFERENCES tmdb_shows (tmdb_id) ON DELETE CASCADE,
  season_number INTEGER NOT NULL CHECK (season_number >= 0),
  name TEXT,
  overview TEXT,
  poster_path TEXT,
  PRIMARY KEY (tmdb_id, season_number)
);

-- Which TMDB show a series folder is. Kept off `series`, because every scan rewrites that table.
-- tmdb_id is null when a search found no exact match.
CREATE TABLE tmdb_matches (
  series_id INTEGER PRIMARY KEY REFERENCES series (id) ON DELETE CASCADE,
  tmdb_id INTEGER REFERENCES tmdb_shows (tmdb_id),
  how TEXT NOT NULL CHECK (how IN ('marker', 'search', 'manual')),
  checked_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX tmdb_matches_tmdb_id ON tmdb_matches (tmdb_id);

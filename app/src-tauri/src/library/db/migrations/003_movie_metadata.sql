-- TMDB metadata for series movies (films tied to a show, e.g. The Transformers: The Movie).

-- What TMDB said about one movie.
CREATE TABLE tmdb_movies (
  tmdb_id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  release_year INTEGER,
  overview TEXT,
  poster_path TEXT,
  logo_path TEXT,
  -- Picked from the poster once it has downloaded, e.g. #8a2b1f.
  spine_colour TEXT,
  fetched_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Which TMDB movie a movie file is. Kept off `movies`, because every scan rewrites that table.
-- tmdb_id is null when a search found no exact match.
CREATE TABLE tmdb_movie_matches (
  movie_id INTEGER PRIMARY KEY REFERENCES movies (id) ON DELETE CASCADE,
  tmdb_id INTEGER REFERENCES tmdb_movies (tmdb_id),
  how TEXT NOT NULL CHECK (how IN ('search', 'manual')),
  checked_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX tmdb_movie_matches_tmdb_id ON tmdb_movie_matches (tmdb_id);

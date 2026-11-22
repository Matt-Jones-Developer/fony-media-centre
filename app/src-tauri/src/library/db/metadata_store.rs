//! Which series need TMDB data, and saving what TMDB sent.

use super::Library;
use crate::metadata::ShowJob;
use crate::metadata::tmdb::ShowDetails;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataCounts {
    pub matched: i64,
    pub unmatched: i64,
    pub waiting: i64,
}

impl Library {
    /// Series with a marker id that has no match yet, or whose marker id has changed since.
    /// With `only`, just that series (fetched again even if it already matches).
    pub fn shows_to_fetch(&self, only: Option<i64>) -> Result<Vec<ShowJob>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT series.id, series.tmdb_id
                 FROM series LEFT JOIN tmdb_matches ON tmdb_matches.series_id = series.id
                 WHERE series.tmdb_id IS NOT NULL
                   AND (?1 IS NULL OR series.id = ?1)
                   AND (?1 IS NOT NULL
                        OR tmdb_matches.series_id IS NULL
                        OR (tmdb_matches.how = 'marker' AND tmdb_matches.tmdb_id IS NOT series.tmdb_id))
                 ORDER BY series.name",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map(params![only], |row| {
                Ok(ShowJob {
                    series_id: row.get(0)?,
                    tmdb_id: row.get(1)?,
                })
            })
            .map_err(|err| err.to_string())?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row.map_err(|err| err.to_string())?);
        }
        Ok(jobs)
    }

    /// Saves the show, replaces its seasons and records the match, all at once.
    pub fn save_show(&mut self, series_id: i64, show: &ShowDetails) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|err| err.to_string())?;
        tx.execute(
            "INSERT INTO tmdb_shows (tmdb_id, name, first_air_year, overview, poster_path, logo_path, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             ON CONFLICT (tmdb_id) DO UPDATE SET
               name = excluded.name, first_air_year = excluded.first_air_year,
               overview = excluded.overview, poster_path = excluded.poster_path,
               logo_path = excluded.logo_path, fetched_at = excluded.fetched_at,
               updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            params![
                show.tmdb_id,
                show.name,
                show.first_air_year,
                show.overview,
                show.poster_path,
                show.logo_path
            ],
        )
        .and_then(|_| {
            tx.execute(
                "DELETE FROM tmdb_seasons WHERE tmdb_id = ?1",
                params![show.tmdb_id],
            )
        })
        .map_err(|err| format!("Can't save a TMDB show: {err}"))?;

        for season in &show.seasons {
            tx.execute(
                "INSERT INTO tmdb_seasons (tmdb_id, season_number, name, overview, poster_path)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    show.tmdb_id,
                    season.number,
                    season.name,
                    season.overview,
                    season.poster_path
                ],
            )
            .map_err(|err| format!("Can't save a TMDB season: {err}"))?;
        }

        tx.execute(
            "INSERT INTO tmdb_matches (series_id, tmdb_id, how, checked_at)
             VALUES (?1, ?2, 'marker', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             ON CONFLICT (series_id) DO UPDATE SET
               tmdb_id = excluded.tmdb_id, how = excluded.how, checked_at = excluded.checked_at,
               updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            params![series_id, show.tmdb_id],
        )
        .map_err(|err| format!("Can't save a TMDB match: {err}"))?;
        tx.commit()
            .map_err(|err| format!("Can't save a TMDB show: {err}"))
    }

    pub fn metadata_counts(&self) -> Result<MetadataCounts, String> {
        let waiting = i64::try_from(self.shows_to_fetch(None)?.len()).unwrap_or(i64::MAX);
        self.conn
            .query_row(
                "SELECT count(tmdb_id), count(*) - count(tmdb_id) FROM tmdb_matches",
                [],
                |row| {
                    Ok(MetadataCounts {
                        matched: row.get(0)?,
                        unmatched: row.get(1)?,
                        waiting,
                    })
                },
            )
            .map_err(|err| format!("Can't count TMDB matches: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::tmdb::SeasonDetails;

    /// One library folder with two series: DuckTales (marker id 720) and a folder with no id.
    fn library_with_two_series() -> Library {
        let library = Library::open_in_memory().unwrap();
        library
            .conn
            .execute_batch(
                "INSERT INTO drives (volume_uuid, name, last_mount_point, last_seen_at)
                   VALUES ('UUID', 'Glitchy_2TB', '/Volumes/Glitchy_2TB', '2026-11-22T00:00:00Z');
                 INSERT INTO library_roots (drive_id, rel_path, kind) VALUES (1, 'MEDIA/Cartoons', 'cartoons');
                 INSERT INTO series (root_id, folder, name, year, tmdb_id, mode, source, is_tidy)
                   VALUES (1, 'DuckTales (1987)', 'DuckTales', 1987, 720, 'tmdb', 'marker', 1),
                          (1, 'Looney Tunes - Golden Collection', 'Looney Tunes', NULL, NULL, 'local', 'parsed', 0);",
            )
            .unwrap();
        library
    }

    fn ducktales(tmdb_id: i64, seasons: u32) -> ShowDetails {
        ShowDetails {
            tmdb_id,
            name: "DuckTales".into(),
            first_air_year: Some(1987),
            overview: Some("Tales of derring-do.".into()),
            poster_path: Some("/poster.jpg".into()),
            logo_path: Some("/logo.png".into()),
            seasons: (1..=seasons)
                .map(|number| SeasonDetails {
                    number,
                    name: Some(format!("Season {number}")),
                    overview: None,
                    poster_path: None,
                })
                .collect(),
        }
    }

    fn season_count(library: &Library) -> i64 {
        library
            .conn
            .query_row("SELECT count(*) FROM tmdb_seasons", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn only_series_with_a_marker_id_need_fetching() {
        let library = library_with_two_series();
        let jobs = library.shows_to_fetch(None).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].series_id, jobs[0].tmdb_id), (1, 720));
        assert_eq!(library.metadata_counts().unwrap().waiting, 1);
    }

    #[test]
    fn a_saved_show_is_not_fetched_again() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 4)).unwrap();

        assert!(library.shows_to_fetch(None).unwrap().is_empty());
        assert_eq!(season_count(&library), 4);
        let counts = library.metadata_counts().unwrap();
        assert_eq!(
            (counts.matched, counts.unmatched, counts.waiting),
            (1, 0, 0)
        );
    }

    #[test]
    fn saving_again_replaces_the_seasons() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 4)).unwrap();
        library.save_show(1, &ducktales(720, 3)).unwrap();
        assert_eq!(season_count(&library), 3);
    }

    #[test]
    fn a_changed_marker_id_is_fetched_again() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 4)).unwrap();
        library
            .conn
            .execute("UPDATE series SET tmdb_id = 721 WHERE id = 1", [])
            .unwrap();
        let jobs = library.shows_to_fetch(None).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].tmdb_id, 721);
    }

    #[test]
    fn asking_for_one_series_fetches_it_even_when_matched() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 4)).unwrap();
        assert_eq!(library.shows_to_fetch(Some(1)).unwrap().len(), 1);
        assert!(library.shows_to_fetch(Some(2)).unwrap().is_empty());
    }
}

//! Which series need TMDB data, and saving what TMDB sent.

use super::Library;
use crate::metadata::ShowJob;
use crate::metadata::art::{ArtImage, SeasonArtRow, SeriesArtRow};
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
               -- A new poster needs its spine colour picked again.
               spine_colour = CASE WHEN tmdb_shows.poster_path IS excluded.poster_path
                                   THEN tmdb_shows.spine_colour END,
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

    /// Every image the tapes use: show and season posters, and logos.
    pub fn art_wanted(&self) -> Result<Vec<ArtImage>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT poster_path, 'poster' FROM tmdb_shows WHERE poster_path IS NOT NULL
                 UNION SELECT poster_path, 'poster' FROM tmdb_seasons WHERE poster_path IS NOT NULL
                 UNION SELECT logo_path, 'logo' FROM tmdb_shows WHERE logo_path IS NOT NULL",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;
        let mut images = Vec::new();
        for row in rows {
            let (tmdb_path, kind) = row.map_err(|err| err.to_string())?;
            if kind == "logo" {
                images.push(ArtImage::logo(tmdb_path));
                continue;
            }
            images.push(ArtImage::poster(tmdb_path));
        }
        Ok(images)
    }

    /// Each matched show with our seasons, joined to TMDB's season of the same number.
    pub fn series_art_rows(&self) -> Result<Vec<SeriesArtRow>, String> {
        let mut shows = self
            .conn
            .prepare(
                "SELECT series.id, series.name, tmdb_shows.overview, tmdb_shows.poster_path,
                        tmdb_shows.logo_path, tmdb_shows.spine_colour
                 FROM series
                 JOIN tmdb_matches ON tmdb_matches.series_id = series.id
                 JOIN tmdb_shows ON tmdb_shows.tmdb_id = tmdb_matches.tmdb_id
                 ORDER BY series.name",
            )
            .map_err(|err| err.to_string())?;
        let show_rows = shows
            .query_map([], |row| {
                Ok(SeriesArtRow {
                    series_id: row.get(0)?,
                    name: row.get(1)?,
                    overview: row.get(2)?,
                    poster_path: row.get(3)?,
                    logo_path: row.get(4)?,
                    spine_colour: row.get(5)?,
                    seasons: Vec::new(),
                })
            })
            .map_err(|err| err.to_string())?;
        let mut all_rows = Vec::new();
        for row in show_rows {
            all_rows.push(row.map_err(|err| err.to_string())?);
        }

        let mut seasons = self
            .conn
            .prepare(
                "SELECT seasons.number, tmdb_seasons.name, tmdb_seasons.overview, tmdb_seasons.poster_path
                 FROM seasons
                 JOIN tmdb_matches ON tmdb_matches.series_id = seasons.series_id
                 LEFT JOIN tmdb_seasons ON tmdb_seasons.tmdb_id = tmdb_matches.tmdb_id
                   AND tmdb_seasons.season_number = seasons.number
                 WHERE seasons.series_id = ?1
                 ORDER BY seasons.number",
            )
            .map_err(|err| err.to_string())?;
        for show in &mut all_rows {
            let season_rows = seasons
                .query_map(params![show.series_id], |row| {
                    Ok(SeasonArtRow {
                        number: row.get(0)?,
                        name: row.get(1)?,
                        overview: row.get(2)?,
                        poster_path: row.get(3)?,
                    })
                })
                .map_err(|err| err.to_string())?;
            for season in season_rows {
                show.seasons.push(season.map_err(|err| err.to_string())?);
            }
        }
        Ok(all_rows)
    }

    /// Shows with a poster but no spine colour yet, as (TMDB id, poster image).
    pub fn posters_needing_colour(&self) -> Result<Vec<(i64, ArtImage)>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT tmdb_id, poster_path FROM tmdb_shows
                 WHERE poster_path IS NOT NULL AND spine_colour IS NULL",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;
        let mut posters = Vec::new();
        for row in rows {
            let (tmdb_id, poster_path) = row.map_err(|err| err.to_string())?;
            posters.push((tmdb_id, ArtImage::poster(poster_path)));
        }
        Ok(posters)
    }

    pub fn save_spine_colour(&mut self, tmdb_id: i64, colour: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tmdb_shows SET spine_colour = ?2,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE tmdb_id = ?1",
                params![tmdb_id, colour],
            )
            .map(|_| ())
            .map_err(|err| format!("Can't save a spine colour: {err}"))
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
    fn wants_every_poster_and_logo_once() {
        let mut library = library_with_two_series();
        let mut show = ducktales(720, 2);
        show.seasons[0].poster_path = Some("/season1.jpg".into());
        library.save_show(1, &show).unwrap();

        let wanted = library.art_wanted().unwrap();
        assert_eq!(wanted.len(), 3);
        assert!(wanted.contains(&ArtImage::poster("/poster.jpg".into())));
        assert!(wanted.contains(&ArtImage::poster("/season1.jpg".into())));
        assert!(wanted.contains(&ArtImage::logo("/logo.png".into())));
    }

    #[test]
    fn joins_our_seasons_to_tmdb_seasons_by_number() {
        let mut library = library_with_two_series();
        let mut show = ducktales(720, 2);
        show.seasons[0].poster_path = Some("/season1.jpg".into());
        library.save_show(1, &show).unwrap();
        library
            .conn
            .execute_batch("INSERT INTO seasons (series_id, number) VALUES (1, 1), (1, 3)")
            .unwrap();

        let rows = library.series_art_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "DuckTales");
        assert_eq!(rows[0].logo_path.as_deref(), Some("/logo.png"));
        let seasons: Vec<(u32, Option<&str>)> = rows[0]
            .seasons
            .iter()
            .map(|season| (season.number, season.poster_path.as_deref()))
            .collect();
        // Season 3 is on the drive but not on TMDB: still listed, with no poster.
        assert_eq!(seasons, vec![(1, Some("/season1.jpg")), (3, None)]);
    }

    #[test]
    fn a_new_poster_needs_its_colour_picked_again() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 1)).unwrap();
        assert_eq!(library.posters_needing_colour().unwrap().len(), 1);

        library.save_spine_colour(720, "#c81e1e").unwrap();
        assert!(library.posters_needing_colour().unwrap().is_empty());
        library.save_show(1, &ducktales(720, 1)).unwrap();
        assert!(library.posters_needing_colour().unwrap().is_empty());

        let mut new_poster = ducktales(720, 1);
        new_poster.poster_path = Some("/new.jpg".into());
        library.save_show(1, &new_poster).unwrap();
        let needing = library.posters_needing_colour().unwrap();
        assert_eq!(needing, vec![(720, ArtImage::poster("/new.jpg".into()))]);
    }

    #[test]
    fn asking_for_one_series_fetches_it_even_when_matched() {
        let mut library = library_with_two_series();
        library.save_show(1, &ducktales(720, 4)).unwrap();
        assert_eq!(library.shows_to_fetch(Some(1)).unwrap().len(), 1);
        assert!(library.shows_to_fetch(Some(2)).unwrap().is_empty());
    }
}

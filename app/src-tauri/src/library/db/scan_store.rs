//! Saves a scan into the library, and reads back the counts and the review queue.
//! Rows are updated in place, so a show or episode keeps its id from one scan to the next.

use super::Library;
use crate::drives::VolumeInfo;
use crate::library::parse::Placement;
use crate::library::scan::{ScanResult, ScannedFile, ScannedSeries};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;

/// A library folder on a drive that's plugged in right now.
pub struct ScanTarget {
    pub root_id: i64,
    pub drive_id: i64,
    /// Relative to the drive, e.g. "MEDIA/Cartoons".
    pub rel_path: String,
    /// Where it is on disk right now.
    pub folder: PathBuf,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    pub series: i64,
    pub untidy_series: i64,
    pub episodes: i64,
    pub movies: i64,
    pub extras: i64,
    pub unplaced_files: i64,
    pub files: i64,
    pub probed_files: i64,
}

impl Library {
    /// The enabled library folders whose drive is in `mounted`.
    pub fn scan_targets(&self, mounted: &[VolumeInfo]) -> Result<Vec<ScanTarget>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT library_roots.id, library_roots.drive_id, library_roots.rel_path, drives.volume_uuid
                 FROM library_roots JOIN drives ON drives.id = library_roots.drive_id
                 WHERE library_roots.enabled = 1",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut targets = Vec::new();
        for row in rows {
            let (root_id, drive_id, rel_path, uuid) = row.map_err(|err| err.to_string())?;
            let Some(volume) = mounted
                .iter()
                .find(|volume| volume.volume_uuid.as_deref() == Some(uuid.as_str()))
            else {
                continue;
            };
            targets.push(ScanTarget {
                root_id,
                drive_id,
                folder: PathBuf::from(&volume.mount_point).join(&rel_path),
                rel_path,
            });
        }
        Ok(targets)
    }

    /// Writes one library folder's scan in a single transaction: all of it or none of it.
    pub fn save_scan(&mut self, target: &ScanTarget, scan: &ScanResult) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|err| err.to_string())?;
        let mut seen_files = HashSet::new();
        let mut seen_series = HashSet::new();

        for file in &scan.loose_files {
            let file_id = save_file(&tx, target.drive_id, file)?;
            clear_placement(&tx, file_id)?;
            seen_files.insert(file_id);
        }
        for series in &scan.series {
            let series_id = save_series(&tx, target.root_id, series)?;
            seen_series.insert(series_id);
            for (file, placement) in &series.files {
                let file_id = save_file(&tx, target.drive_id, file)?;
                save_placement(&tx, series_id, file_id, placement)?;
                seen_files.insert(file_id);
            }
        }

        remove_unseen_files(&tx, target, &seen_files)?;
        remove_unseen_series(&tx, target.root_id, &seen_series)?;
        remove_empty_seasons(&tx, target.root_id)?;
        tx.commit()
            .map_err(|err| format!("Can't save the scan: {err}"))
    }

    pub fn counts(&self) -> Result<LibraryCounts, String> {
        self.conn
            .query_row(
                "SELECT
                   (SELECT count(*) FROM series),
                   (SELECT count(*) FROM series WHERE is_tidy = 0),
                   (SELECT count(*) FROM episodes),
                   (SELECT count(*) FROM movies),
                   (SELECT count(*) FROM extras),
                   (SELECT count(*) FROM files WHERE id NOT IN
                      (SELECT file_id FROM episodes UNION SELECT file_id FROM movies UNION SELECT file_id FROM extras)),
                   (SELECT count(*) FROM files),
                   (SELECT count(*) FROM files WHERE probed_at IS NOT NULL)",
                [],
                |row| {
                    Ok(LibraryCounts {
                        series: row.get(0)?,
                        untidy_series: row.get(1)?,
                        episodes: row.get(2)?,
                        movies: row.get(3)?,
                        extras: row.get(4)?,
                        unplaced_files: row.get(5)?,
                        files: row.get(6)?,
                        probed_files: row.get(7)?,
                    })
                },
            )
            .map_err(|err| format!("Can't count the library: {err}"))
    }

    /// Paths (relative to their drive) of files the scanner couldn't place.
    pub fn review_queue(&self) -> Result<Vec<String>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT rel_path FROM files WHERE id NOT IN
                   (SELECT file_id FROM episodes UNION SELECT file_id FROM movies UNION SELECT file_id FROM extras)
                 ORDER BY rel_path",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| row.get(0))
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<_, _>>()
            .map_err(|err| format!("Can't read the review queue: {err}"))
    }
}

/// Inserts or updates one file and returns its id.
fn save_file(conn: &Connection, drive_id: i64, file: &ScannedFile) -> Result<i64, String> {
    // A file whose size or modified time changed is a different file, so its ffprobe data is cleared.
    conn.prepare_cached(
        "INSERT INTO files (drive_id, rel_path, size, modified_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', ?4, 'unixepoch'))
         ON CONFLICT (drive_id, rel_path) DO UPDATE SET
           size = excluded.size,
           modified_at = excluded.modified_at,
           duration_ms = NULL, video_codec = NULL, width = NULL, height = NULL, probed_at = NULL,
           updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE files.size <> excluded.size OR files.modified_at <> excluded.modified_at",
    )
    .and_then(|mut insert| {
        insert.execute(params![
            drive_id,
            file.rel_path,
            file.size,
            file.modified_secs
        ])
    })
    .map_err(|err| format!("Can't save {}: {err}", file.rel_path))?;

    conn.prepare_cached("SELECT id FROM files WHERE drive_id = ?1 AND rel_path = ?2")
        .and_then(|mut select| select.query_row(params![drive_id, file.rel_path], |row| row.get(0)))
        .map_err(|err| format!("Can't find {}: {err}", file.rel_path))
}

fn save_series(conn: &Connection, root_id: i64, series: &ScannedSeries) -> Result<i64, String> {
    conn.prepare_cached(
        "INSERT INTO series (root_id, folder, name, year, tmdb_id, mode, source, is_tidy)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (root_id, folder) DO UPDATE SET
           name = excluded.name, year = excluded.year, tmdb_id = excluded.tmdb_id,
           mode = excluded.mode, source = excluded.source, is_tidy = excluded.is_tidy,
           updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE (series.name, series.year, series.tmdb_id, series.mode, series.source, series.is_tidy)
           IS NOT (excluded.name, excluded.year, excluded.tmdb_id, excluded.mode, excluded.source, excluded.is_tidy)",
    )
    .and_then(|mut insert| {
        insert.execute(params![
            root_id,
            series.folder,
            series.name,
            series.year,
            series.tmdb_id,
            series.mode.as_str(),
            series.source.as_str(),
            series.is_tidy(),
        ])
    })
    .map_err(|err| format!("Can't save {}: {err}", series.folder))?;

    conn.prepare_cached("SELECT id FROM series WHERE root_id = ?1 AND folder = ?2")
        .and_then(|mut select| select.query_row(params![root_id, series.folder], |row| row.get(0)))
        .map_err(|err| format!("Can't find {}: {err}", series.folder))
}

fn season_id(conn: &Connection, series_id: i64, number: u32) -> Result<i64, String> {
    conn.prepare_cached(
        "INSERT INTO seasons (series_id, number) VALUES (?1, ?2)
         ON CONFLICT (series_id, number) DO NOTHING",
    )
    .and_then(|mut insert| insert.execute(params![series_id, number]))
    .and_then(|_| {
        conn.prepare_cached("SELECT id FROM seasons WHERE series_id = ?1 AND number = ?2")
    })
    .and_then(|mut select| select.query_row(params![series_id, number], |row| row.get(0)))
    .map_err(|err| format!("Can't save season {number}: {err}"))
}

/// A file is an episode, a movie or an extra, never two at once, so the other two are cleared first.
fn save_placement(
    conn: &Connection,
    series_id: i64,
    file_id: i64,
    placement: &Placement,
) -> Result<(), String> {
    match placement {
        Placement::Episode(episode) => {
            let season_id = season_id(conn, series_id, episode.season)?;
            clear_tables(conn, file_id, &["movies", "extras"])?;
            conn.prepare_cached(
                "INSERT INTO episodes (season_id, file_id, episode, episode_end, segment, title)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (file_id) DO UPDATE SET
                   season_id = excluded.season_id, episode = excluded.episode,
                   episode_end = excluded.episode_end, segment = excluded.segment, title = excluded.title,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE (episodes.season_id, episodes.episode, episodes.episode_end, episodes.segment, episodes.title)
                   IS NOT (excluded.season_id, excluded.episode, excluded.episode_end, excluded.segment, excluded.title)",
            )
            .and_then(|mut insert| {
                insert.execute(params![
                    season_id,
                    file_id,
                    episode.episode,
                    episode.episode_end,
                    episode.segment.map(String::from),
                    episode.title,
                ])
            })
            .map_err(|err| format!("Can't save an episode: {err}"))?;
        }
        Placement::Movie { title, year } => {
            clear_tables(conn, file_id, &["episodes", "extras"])?;
            conn.prepare_cached(
                "INSERT INTO movies (series_id, file_id, title, year) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (file_id) DO UPDATE SET
                   series_id = excluded.series_id, title = excluded.title, year = excluded.year,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE (movies.series_id, movies.title, movies.year)
                   IS NOT (excluded.series_id, excluded.title, excluded.year)",
            )
            .and_then(|mut insert| insert.execute(params![series_id, file_id, title, year]))
            .map_err(|err| format!("Can't save a movie: {err}"))?;
        }
        Placement::Extra { title } => {
            clear_tables(conn, file_id, &["episodes", "movies"])?;
            conn.prepare_cached(
                "INSERT INTO extras (series_id, file_id, title) VALUES (?1, ?2, ?3)
                 ON CONFLICT (file_id) DO UPDATE SET
                   series_id = excluded.series_id, title = excluded.title,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE (extras.series_id, extras.title) IS NOT (excluded.series_id, excluded.title)",
            )
            .and_then(|mut insert| insert.execute(params![series_id, file_id, title]))
            .map_err(|err| format!("Can't save an extra: {err}"))?;
        }
        Placement::Unplaced => clear_placement(conn, file_id)?,
    }
    Ok(())
}

fn clear_placement(conn: &Connection, file_id: i64) -> Result<(), String> {
    clear_tables(conn, file_id, &["episodes", "movies", "extras"])
}

/// `tables` only ever holds the fixed names above, never outside input.
fn clear_tables(conn: &Connection, file_id: i64, tables: &[&str]) -> Result<(), String> {
    for table in tables {
        conn.prepare_cached(&format!("DELETE FROM {table} WHERE file_id = ?1"))
            .and_then(|mut delete| delete.execute([file_id]))
            .map_err(|err| format!("Can't clear {table}: {err}"))?;
    }
    Ok(())
}

/// Files under this library folder that the scan didn't find are gone from the drive.
fn remove_unseen_files(
    conn: &Connection,
    target: &ScanTarget,
    seen: &HashSet<i64>,
) -> Result<(), String> {
    let mut query = conn
        .prepare(
            "SELECT id FROM files
             WHERE drive_id = ?1 AND substr(rel_path, 1, length(?2) + 1) = ?2 || '/'",
        )
        .map_err(|err| err.to_string())?;
    let ids: Vec<i64> = query
        .query_map(params![target.drive_id, target.rel_path], |row| row.get(0))
        .and_then(|rows| rows.collect())
        .map_err(|err| err.to_string())?;
    for id in ids {
        if seen.contains(&id) {
            continue;
        }
        conn.prepare_cached("DELETE FROM files WHERE id = ?1")
            .and_then(|mut delete| delete.execute([id]))
            .map_err(|err| format!("Can't remove a missing file: {err}"))?;
    }
    Ok(())
}

fn remove_unseen_series(
    conn: &Connection,
    root_id: i64,
    seen: &HashSet<i64>,
) -> Result<(), String> {
    let mut query = conn
        .prepare("SELECT id FROM series WHERE root_id = ?1")
        .map_err(|err| err.to_string())?;
    let ids: Vec<i64> = query
        .query_map([root_id], |row| row.get(0))
        .and_then(|rows| rows.collect())
        .map_err(|err| err.to_string())?;
    for id in ids {
        if seen.contains(&id) {
            continue;
        }
        conn.prepare_cached("DELETE FROM series WHERE id = ?1")
            .and_then(|mut delete| delete.execute([id]))
            .map_err(|err| format!("Can't remove a missing show: {err}"))?;
    }
    Ok(())
}

fn remove_empty_seasons(conn: &Connection, root_id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM seasons
         WHERE series_id IN (SELECT id FROM series WHERE root_id = ?1)
           AND NOT EXISTS (SELECT 1 FROM episodes WHERE episodes.season_id = seasons.id)",
        [root_id],
    )
    .map(|_| ())
    .map_err(|err| format!("Can't remove empty seasons: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::parse::Episode;
    use crate::library::scan::{SeriesMode, SeriesSource};

    fn media_drive() -> VolumeInfo {
        VolumeInfo {
            mount_point: "/Volumes/Glitchy_2TB".into(),
            volume_name: Some("Glitchy_2TB".into()),
            volume_uuid: Some("D2946B44-EEF5-3A81-A7BE-648CD3312449".into()),
        }
    }

    fn library_with_cartoons() -> (Library, ScanTarget) {
        let mut library = Library::open_in_memory().unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();
        let target = library.scan_targets(&[media_drive()]).unwrap().remove(0);
        (library, target)
    }

    fn file(name: &str, size: i64) -> ScannedFile {
        ScannedFile {
            rel_path: format!("MEDIA/Cartoons/DuckTales (1987)/{name}"),
            size,
            modified_secs: 1_700_000_000,
        }
    }

    fn episode(number: u32) -> Placement {
        Placement::Episode(Episode {
            season: 1,
            episode: number,
            episode_end: None,
            segment: None,
            title: Some(format!("Episode {number}")),
        })
    }

    fn ducktales(files: Vec<(ScannedFile, Placement)>) -> ScanResult {
        ScanResult {
            series: vec![ScannedSeries {
                folder: "DuckTales (1987)".into(),
                name: "DuckTales".into(),
                year: Some(1987),
                tmdb_id: Some(4366),
                mode: SeriesMode::Tmdb,
                source: SeriesSource::Marker,
                files,
            }],
            loose_files: Vec::new(),
        }
    }

    fn episode_ids(library: &Library) -> Vec<i64> {
        let mut query = library
            .conn
            .prepare("SELECT id FROM episodes ORDER BY id")
            .unwrap();
        query
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn finds_targets_only_on_mounted_drives() {
        let (library, target) = library_with_cartoons();
        assert_eq!(
            target.folder,
            PathBuf::from("/Volumes/Glitchy_2TB/MEDIA/Cartoons")
        );
        assert!(library.scan_targets(&[]).unwrap().is_empty());
    }

    #[test]
    fn saving_the_same_scan_twice_changes_nothing() {
        let (mut library, target) = library_with_cartoons();
        let scan = || {
            ducktales(vec![
                (file("Season 01/E1.mkv", 100), episode(1)),
                (file("Season 01/E2.mkv", 100), episode(2)),
                (file("odd.mkv", 100), Placement::Unplaced),
            ])
        };
        library.save_scan(&target, &scan()).unwrap();
        let first_ids = episode_ids(&library);
        library.save_scan(&target, &scan()).unwrap();

        assert_eq!(episode_ids(&library), first_ids);
        assert_eq!(
            library.counts().unwrap(),
            LibraryCounts {
                series: 1,
                untidy_series: 1,
                episodes: 2,
                movies: 0,
                extras: 0,
                unplaced_files: 1,
                files: 3,
                probed_files: 0,
            }
        );
        assert_eq!(
            library.review_queue().unwrap(),
            vec!["MEDIA/Cartoons/DuckTales (1987)/odd.mkv"]
        );
    }

    #[test]
    fn a_file_gone_from_the_drive_is_removed_with_its_episode() {
        let (mut library, target) = library_with_cartoons();
        library
            .save_scan(
                &target,
                &ducktales(vec![
                    (file("Season 01/E1.mkv", 100), episode(1)),
                    (file("Season 01/E2.mkv", 100), episode(2)),
                ]),
            )
            .unwrap();
        library
            .save_scan(
                &target,
                &ducktales(vec![(file("Season 01/E1.mkv", 100), episode(1))]),
            )
            .unwrap();

        let counts = library.counts().unwrap();
        assert_eq!((counts.episodes, counts.unplaced_files), (1, 0));
    }

    #[test]
    fn a_show_gone_from_the_drive_is_removed() {
        let (mut library, target) = library_with_cartoons();
        library
            .save_scan(
                &target,
                &ducktales(vec![(file("Season 01/E1.mkv", 100), episode(1))]),
            )
            .unwrap();
        let empty = ScanResult {
            series: Vec::new(),
            loose_files: Vec::new(),
        };
        library.save_scan(&target, &empty).unwrap();

        let counts = library.counts().unwrap();
        assert_eq!((counts.series, counts.episodes), (0, 0));
        let seasons: i64 = library
            .conn
            .query_row("SELECT count(*) FROM seasons", [], |row| row.get(0))
            .unwrap();
        assert_eq!(seasons, 0);
    }

    #[test]
    fn a_changed_file_loses_its_ffprobe_data() {
        let (mut library, target) = library_with_cartoons();
        let save = |library: &mut Library, size| {
            library
                .save_scan(
                    &target,
                    &ducktales(vec![(file("Season 01/E1.mkv", size), episode(1))]),
                )
                .unwrap()
        };
        let probed_at = |library: &Library| -> Option<String> {
            library
                .conn
                .query_row("SELECT probed_at FROM files", [], |row| row.get(0))
                .unwrap()
        };

        save(&mut library, 100);
        library
            .conn
            .execute(
                "UPDATE files SET probed_at = '2026-11-22T00:00:00Z', duration_ms = 1000",
                [],
            )
            .unwrap();
        save(&mut library, 100);
        assert!(probed_at(&library).is_some());

        save(&mut library, 200);
        assert_eq!(probed_at(&library), None);
    }

    #[test]
    fn a_file_can_move_from_unplaced_to_an_episode() {
        let (mut library, target) = library_with_cartoons();
        library
            .save_scan(
                &target,
                &ducktales(vec![(file("Season 01/E1.mkv", 100), Placement::Unplaced)]),
            )
            .unwrap();
        library
            .save_scan(
                &target,
                &ducktales(vec![(file("Season 01/E1.mkv", 100), episode(1))]),
            )
            .unwrap();

        let counts = library.counts().unwrap();
        assert_eq!(
            (counts.episodes, counts.unplaced_files, counts.untidy_series),
            (1, 0, 0)
        );
    }
}

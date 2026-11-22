//! TMDB metadata and art for the shelf. See docs/ARCHITECTURE.md "Metadata and art".

pub mod art;
pub mod matching;
pub mod tmdb;

use std::thread;
use tmdb::{MovieDetails, ShowDetails, Tmdb, TmdbError};

/// How a series folder was matched to its TMDB show.
#[derive(Clone, Copy)]
pub enum MatchedBy {
    Marker,
    Search,
}

impl MatchedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchedBy::Marker => "marker",
            MatchedBy::Search => "search",
        }
    }
}

/// A series folder with no marker id, to look up by name and year.
pub struct SearchJob {
    pub series_id: i64,
    pub name: String,
    pub year: Option<i32>,
}

/// A series movie to look up by title and year.
pub struct MovieJob {
    pub movie_id: i64,
    pub title: String,
    pub year: Option<i32>,
}

/// Searches TMDB for a series movie, then fetches it if exactly one title matches.
/// `Ok(None)` means no clear match.
pub fn search_movie(tmdb: &Tmdb, job: &MovieJob) -> Result<Option<MovieDetails>, TmdbError> {
    let Some(year) = job.year else {
        return Ok(None);
    };
    let results = tmdb.search_movie(&job.title, year)?;
    let Some(tmdb_id) = matching::pick_exact_title(&job.title, &results) else {
        return Ok(None);
    };
    tmdb.movie(tmdb_id).map(Some)
}

/// A series folder whose TMDB show needs fetching.
pub struct ShowJob {
    pub series_id: i64,
    pub tmdb_id: i64,
}

pub struct ShowResult {
    pub series_id: i64,
    pub result: Result<ShowDetails, TmdbError>,
}

/// Searches TMDB for a folder by name and year, then fetches the show if exactly one result matches.
/// `Ok(None)` means no clear match.
pub fn search_show(tmdb: &Tmdb, job: &SearchJob) -> Result<Option<ShowDetails>, TmdbError> {
    let Some(year) = job.year else {
        return Ok(None);
    };
    let results = tmdb.search_tv(&job.name, year)?;
    let Some(tmdb_id) = matching::pick_exact(&job.name, job.year, &results) else {
        return Ok(None);
    };
    tmdb.show(tmdb_id).map(Some)
}

/// Fetches `jobs` with `workers` requests running at once.
/// Each worker stops early once something fails that would fail every request (no key, offline).
pub fn fetch_shows(tmdb: &Tmdb, jobs: &[ShowJob], workers: usize) -> Vec<ShowResult> {
    let chunk_size = jobs.len().div_ceil(workers.max(1)).max(1);
    let mut fetched = Vec::new();
    thread::scope(|scope| {
        let running: Vec<_> = jobs
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || fetch_chunk(tmdb, chunk)))
            .collect();
        for worker in running {
            if let Ok(results) = worker.join() {
                fetched.extend(results);
            }
        }
    });
    fetched
}

fn fetch_chunk(tmdb: &Tmdb, jobs: &[ShowJob]) -> Vec<ShowResult> {
    let mut results = Vec::new();
    for job in jobs {
        let result = tmdb.show(job.tmdb_id);
        let should_stop = result.as_ref().is_err_and(|err| err.stops_everything());
        results.push(ShowResult {
            series_id: job.series_id,
            result,
        });
        if should_stop {
            break;
        }
    }
    results
}

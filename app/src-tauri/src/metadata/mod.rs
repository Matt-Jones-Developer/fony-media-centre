//! TMDB metadata and art for the shelf. See docs/ARCHITECTURE.md "Metadata and art".

pub mod tmdb;

use std::thread;
use tmdb::{ShowDetails, Tmdb, TmdbError};

/// A series folder whose TMDB show needs fetching.
pub struct ShowJob {
    pub series_id: i64,
    pub tmdb_id: i64,
}

pub struct ShowResult {
    pub series_id: i64,
    pub result: Result<ShowDetails, TmdbError>,
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

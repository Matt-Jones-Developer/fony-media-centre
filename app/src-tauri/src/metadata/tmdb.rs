//! The only code that talks to TMDB. Nothing else imports the HTTP client.
//! The key is read here and never leaves: error messages and logs never include it or a full URL.

use serde::Deserialize;
use std::fmt;
use std::fs;
use std::thread;
use std::time::Duration;

pub struct Tmdb {
    agent: ureq::Agent,
    key: String,
}

/// What the tapes need from one TMDB show.
#[derive(Debug, PartialEq)]
pub struct ShowDetails {
    pub tmdb_id: i64,
    pub name: String,
    pub first_air_year: Option<i32>,
    pub overview: Option<String>,
    /// TMDB's image path, e.g. `/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg`.
    pub poster_path: Option<String>,
    /// The transparent title logo, for the spine.
    pub logo_path: Option<String>,
    pub seasons: Vec<SeasonDetails>,
}

#[derive(Debug, PartialEq)]
pub struct SeasonDetails {
    /// 0 is Specials.
    pub number: u32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

/// What a movie tape needs from TMDB.
#[derive(Debug, PartialEq)]
pub struct MovieDetails {
    pub tmdb_id: i64,
    pub title: String,
    pub release_year: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub logo_path: Option<String>,
}

/// One movie search result: just enough to decide whether it's the same film.
#[derive(Debug, PartialEq)]
pub struct MovieSearchResult {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
}

/// One TV search result: just enough to decide whether it's the same show.
#[derive(Debug, PartialEq)]
pub struct SearchResult {
    pub tmdb_id: i64,
    pub name: String,
    pub original_name: Option<String>,
    pub first_air_year: Option<i32>,
}

#[derive(Debug, PartialEq)]
pub enum TmdbError {
    KeyMissing,
    KeyRejected,
    NotFound,
    Offline,
    TooManyRequests,
    Failed(u16),
    BadAnswer,
}

impl TmdbError {
    /// True when nothing else will work either, so the whole fetch should stop.
    pub fn stops_everything(&self) -> bool {
        matches!(
            self,
            TmdbError::KeyMissing | TmdbError::KeyRejected | TmdbError::Offline
        )
    }
}

impl fmt::Display for TmdbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TmdbError::KeyMissing => write!(f, "No TMDB key. Add TMDB_API_KEY to .env.local"),
            TmdbError::KeyRejected => write!(f, "TMDB rejected the key"),
            TmdbError::NotFound => write!(f, "TMDB couldn't find it"),
            TmdbError::Offline => write!(f, "Can't reach TMDB (offline?)"),
            TmdbError::TooManyRequests => write!(f, "TMDB is busy; try again later"),
            TmdbError::Failed(status) => write!(f, "TMDB answered with error {status}"),
            TmdbError::BadAnswer => write!(f, "TMDB sent something we couldn't read"),
        }
    }
}

impl Tmdb {
    /// The key comes from `TMDB_API_KEY`, or in development from the repo's `.env.local`.
    pub fn from_env() -> Result<Self, TmdbError> {
        let key = std::env::var("TMDB_API_KEY")
            .ok()
            .or_else(key_from_env_file)
            .filter(|key| !key.is_empty())
            .ok_or(TmdbError::KeyMissing)?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            // Status codes come back as normal answers, so they can be checked one by one.
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Tmdb { agent, key })
    }

    /// One call gets the show, its seasons and its logos.
    pub fn show(&self, tmdb_id: i64) -> Result<ShowDetails, TmdbError> {
        let raw: RawShow = self.get(
            &format!("/tv/{tmdb_id}"),
            &[
                ("append_to_response", "images"),
                ("include_image_language", "en,null"),
            ],
        )?;
        Ok(show_details(raw))
    }

    /// TV shows called `name` that first aired in `year`. Only the first page: an exact name and year rarely gives more.
    pub fn search_tv(&self, name: &str, year: i32) -> Result<Vec<SearchResult>, TmdbError> {
        let year = year.to_string();
        let page: RawSearchPage = self.get(
            "/search/tv",
            &[("query", name), ("first_air_date_year", year.as_str())],
        )?;
        let mut results = Vec::new();
        for raw in page.results {
            results.push(SearchResult {
                tmdb_id: raw.id,
                name: raw.name,
                original_name: non_empty(raw.original_name),
                first_air_year: year_of(raw.first_air_date.as_deref()),
            });
        }
        Ok(results)
    }

    /// Movies called `title` with any release in `year`. TMDB's main date can be a later
    /// release elsewhere (Hey Arnold! The Jungle Movie aired in 2017 but is dated 2018), so the year is checked here.
    pub fn search_movie(
        &self,
        title: &str,
        year: i32,
    ) -> Result<Vec<MovieSearchResult>, TmdbError> {
        let year = year.to_string();
        let page: RawMovieSearchPage = self.get(
            "/search/movie",
            &[("query", title), ("year", year.as_str())],
        )?;
        let mut results = Vec::new();
        for raw in page.results {
            results.push(MovieSearchResult {
                tmdb_id: raw.id,
                title: raw.title,
                original_title: non_empty(raw.original_title),
            });
        }
        Ok(results)
    }

    /// One call gets the movie and its logos.
    pub fn movie(&self, tmdb_id: i64) -> Result<MovieDetails, TmdbError> {
        let raw: RawMovie = self.get(
            &format!("/movie/{tmdb_id}"),
            &[
                ("append_to_response", "images"),
                ("include_image_language", "en,null"),
            ],
        )?;
        let logos = raw.images.map(|images| images.logos).unwrap_or_default();
        Ok(MovieDetails {
            tmdb_id: raw.id,
            title: raw.title,
            release_year: year_of(raw.release_date.as_deref()),
            overview: non_empty(raw.overview),
            poster_path: non_empty(raw.poster_path),
            logo_path: best_logo(&logos),
        })
    }

    /// One image from TMDB's image server, e.g. size `w780` and path `/abc.jpg`. Needs no key.
    pub fn image(&self, size: &str, tmdb_path: &str) -> Result<Vec<u8>, TmdbError> {
        let url = format!("https://image.tmdb.org/t/p/{size}{tmdb_path}");
        let mut response = self.agent.get(url).call().map_err(|_| TmdbError::Offline)?;
        match response.status().as_u16() {
            200 => response
                .body_mut()
                .read_to_vec()
                .map_err(|_| TmdbError::BadAnswer),
            404 => Err(TmdbError::NotFound),
            429 => Err(TmdbError::TooManyRequests),
            status => Err(TmdbError::Failed(status)),
        }
    }

    /// Waits 1, 2, then 4 seconds when TMDB says it's busy, then gives up.
    fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, TmdbError> {
        let api = "https://api.themoviedb.org/3";
        let waits = [1, 2, 4];
        for attempt in 0..=waits.len() {
            let mut request = self
                .agent
                .get(format!("{api}{path}"))
                .query("api_key", &self.key)
                .query("language", "en-GB");
            for (name, value) in params {
                request = request.query(*name, *value);
            }
            // ureq's own error text includes the URL, and the URL includes the key, so it's dropped here.
            let mut response = request.call().map_err(|_| TmdbError::Offline)?;
            match response.status().as_u16() {
                200 => {
                    return response
                        .body_mut()
                        .read_json()
                        .map_err(|_| TmdbError::BadAnswer);
                }
                401 => return Err(TmdbError::KeyRejected),
                404 => return Err(TmdbError::NotFound),
                429 => {
                    let Some(seconds) = waits.get(attempt) else {
                        return Err(TmdbError::TooManyRequests);
                    };
                    thread::sleep(Duration::from_secs(*seconds));
                }
                status => return Err(TmdbError::Failed(status)),
            }
        }
        Err(TmdbError::TooManyRequests)
    }
}

/// Reads `TMDB_API_KEY=...` from `.env.local` at the repo root. Only there in development.
fn key_from_env_file() -> Option<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env.local");
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let Some(value) = line.trim().strip_prefix("TMDB_API_KEY=") else {
            continue;
        };
        return Some(value.trim().trim_matches(['"', '\'']).to_string());
    }
    None
}

#[derive(Deserialize)]
struct RawMovieSearchPage {
    #[serde(default)]
    results: Vec<RawMovieSearchResult>,
}

#[derive(Deserialize)]
struct RawMovieSearchResult {
    id: i64,
    title: String,
    original_title: Option<String>,
}

#[derive(Deserialize)]
struct RawMovie {
    id: i64,
    title: String,
    release_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    images: Option<RawImages>,
}

#[derive(Deserialize)]
struct RawSearchPage {
    #[serde(default)]
    results: Vec<RawSearchResult>,
}

#[derive(Deserialize)]
struct RawSearchResult {
    id: i64,
    name: String,
    original_name: Option<String>,
    first_air_date: Option<String>,
}

#[derive(Deserialize)]
struct RawShow {
    id: i64,
    name: String,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    #[serde(default)]
    seasons: Vec<RawSeason>,
    images: Option<RawImages>,
}

#[derive(Deserialize)]
struct RawSeason {
    season_number: u32,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

#[derive(Deserialize)]
struct RawImages {
    #[serde(default)]
    logos: Vec<RawLogo>,
}

#[derive(Deserialize)]
struct RawLogo {
    file_path: String,
    iso_639_1: Option<String>,
    vote_average: f64,
}

fn show_details(raw: RawShow) -> ShowDetails {
    let logos = raw.images.map(|images| images.logos).unwrap_or_default();
    let mut seasons = Vec::new();
    for season in raw.seasons {
        seasons.push(SeasonDetails {
            number: season.season_number,
            name: non_empty(season.name),
            overview: non_empty(season.overview),
            poster_path: non_empty(season.poster_path),
        });
    }
    ShowDetails {
        tmdb_id: raw.id,
        name: raw.name,
        first_air_year: year_of(raw.first_air_date.as_deref()),
        overview: non_empty(raw.overview),
        poster_path: non_empty(raw.poster_path),
        logo_path: best_logo(&logos),
        seasons,
    }
}

/// "1987-09-18" to 1987. TMDB sends "" for an unknown date.
fn year_of(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

/// English first, then PNG over SVG, then the best voted.
fn best_logo(logos: &[RawLogo]) -> Option<String> {
    let mut best: Option<&RawLogo> = None;
    for logo in logos {
        let Some(current) = best else {
            best = Some(logo);
            continue;
        };
        if logo_rank(logo) > logo_rank(current) {
            best = Some(logo);
        }
    }
    best.map(|logo| logo.file_path.clone())
}

fn logo_rank(logo: &RawLogo) -> (bool, bool, f64) {
    (
        logo.iso_639_1.as_deref() == Some("en"),
        logo.file_path.ends_with(".png"),
        logo.vote_average,
    )
}

/// TMDB sends "" as often as null for a missing value.
fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ducktales() -> ShowDetails {
        let raw: RawShow = serde_json::from_str(include_str!("fixtures/ducktales.json")).unwrap();
        show_details(raw)
    }

    #[test]
    fn reads_a_real_show() {
        let show = ducktales();
        assert_eq!(show.tmdb_id, 720);
        assert_eq!(show.name, "DuckTales");
        assert_eq!(show.first_air_year, Some(1987));
        assert_eq!(
            show.poster_path.as_deref(),
            Some("/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg")
        );
        assert!(show.overview.is_some());
        assert_eq!(show.seasons.len(), 4);
        assert_eq!(
            show.seasons[0].poster_path.as_deref(),
            Some("/dNBaXrbAMmfKcZv8kAAsV8ynPC4.jpg")
        );
    }

    #[test]
    fn picks_the_best_voted_english_logo() {
        assert_eq!(
            ducktales().logo_path.as_deref(),
            Some("/rx0din6FtPgTy5ZRAI2JuOCH283.png")
        );
    }

    #[test]
    fn prefers_english_then_png_over_votes() {
        let logo = |file_path: &str, language: Option<&str>, vote_average: f64| RawLogo {
            file_path: file_path.into(),
            iso_639_1: language.map(String::from),
            vote_average,
        };
        let logos = [
            logo("/no-language.png", None, 9.0),
            logo("/english.svg", Some("en"), 8.0),
            logo("/english.png", Some("en"), 1.0),
        ];
        assert_eq!(best_logo(&logos).as_deref(), Some("/english.png"));
        assert_eq!(best_logo(&[]), None);
    }

    #[test]
    fn treats_empty_text_as_missing() {
        let raw: RawShow = serde_json::from_str(
            r#"{"id": 1, "name": "Show", "first_air_date": "", "overview": "", "poster_path": null,
                "seasons": [{"season_number": 1, "name": "Season 1", "overview": "", "poster_path": null}]}"#,
        )
        .unwrap();
        let show = show_details(raw);
        assert_eq!(show.first_air_year, None);
        assert_eq!(show.overview, None);
        assert_eq!(show.logo_path, None);
        assert_eq!(show.seasons[0].overview, None);
    }

    /// Calls TMDB for real: `cargo test -- --ignored`. Needs the network and the key.
    #[test]
    #[ignore]
    fn fetches_ducktales_from_tmdb() {
        let show = Tmdb::from_env().unwrap().show(720).unwrap();
        assert_eq!(show.name, "DuckTales");
        assert!(show.logo_path.is_some());
        assert_eq!(
            Tmdb::from_env().unwrap().show(999_999_999),
            Err(TmdbError::NotFound)
        );
    }

    /// Calls TMDB for real: `cargo test -- --ignored`. Needs the network and the key.
    #[test]
    #[ignore]
    fn searches_tmdb_by_name_and_year() {
        let results = Tmdb::from_env()
            .unwrap()
            .search_tv("Dungeons & Dragons", 1983)
            .unwrap();
        assert!(results.iter().any(|result| result.tmdb_id == 553));
    }

    /// Calls TMDB for real: `cargo test -- --ignored`. Needs the network and the key.
    #[test]
    #[ignore]
    fn finds_a_movie_by_any_release_that_year() {
        let tmdb = Tmdb::from_env().unwrap();
        let results = tmdb
            .search_movie("Hey Arnold! The Jungle Movie", 2017)
            .unwrap();
        assert!(results.iter().any(|result| result.tmdb_id == 439_058));
        let movie = tmdb.movie(1857).unwrap();
        assert_eq!(movie.title, "The Transformers: The Movie");
        assert!(movie.poster_path.is_some());
    }

    /// Calls TMDB for real: `cargo test -- --ignored`. Needs the network and the key.
    #[test]
    #[ignore]
    fn downloads_a_real_poster() {
        let bytes = Tmdb::from_env()
            .unwrap()
            .image("w780", "/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg")
            .unwrap();
        // Every JPEG starts with these two bytes.
        assert_eq!(bytes.get(..2), Some(&[0xFF, 0xD8][..]));
    }
}

//! The art folder in Application Support: where each TMDB image lives, and saving downloads safely.
//! A file that exists is never downloaded again, because TMDB never reuses an image's file name.

use super::tmdb::Tmdb;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

/// One TMDB image at one size, e.g. `w780` and `/abc.jpg`.
#[derive(Debug, PartialEq)]
pub struct ArtImage {
    pub size: &'static str,
    pub tmdb_path: String,
}

impl ArtImage {
    /// Posters are shown big on the wall, so they come larger than logos.
    pub fn poster(tmdb_path: String) -> Self {
        ArtImage {
            size: "w780",
            tmdb_path,
        }
    }

    pub fn logo(tmdb_path: String) -> Self {
        ArtImage {
            size: "w500",
            tmdb_path,
        }
    }
}

/// Whose spine a poster colours.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PosterOwner {
    /// A TMDB show id.
    Show(i64),
    /// A TMDB movie id.
    Movie(i64),
}

/// Where `image` lives on disk, e.g. `art/w780/abc.jpg`.
/// `None` for a name that isn't a plain TMDB file name, so nothing can be written outside the folder.
pub fn local_path(art_folder: &Path, image: &ArtImage) -> Option<PathBuf> {
    let file_name = image.tmdb_path.strip_prefix('/')?;
    let (stem, extension) = file_name.split_once('.')?;
    let is_plain_stem = !stem.is_empty() && stem.chars().all(|c| c.is_ascii_alphanumeric());
    if !is_plain_stem || !["jpg", "png", "svg"].contains(&extension) {
        return None;
    }
    Some(art_folder.join(image.size).join(file_name))
}

/// The file on disk for `image`, as text for the UI, but only once it has downloaded.
pub fn existing_file(art_folder: &Path, image: ArtImage) -> Option<String> {
    let path = local_path(art_folder, &image)?;
    if !path.is_file() {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

/// Writes to a `.part` file first and renames it, so a failed download never leaves half an image.
pub fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let folder = path.parent().ok_or("An art path has no folder")?;
    fs::create_dir_all(folder).map_err(|err| format!("Can't create the art folder: {err}"))?;
    let partial = path.with_extension("part");
    fs::write(&partial, bytes)
        .and_then(|_| fs::rename(&partial, path))
        .map_err(|err| format!("Can't save an image: {err}"))
}

/// The poster's most common strong colour, e.g. `#8a2b1f`, for the spine label.
/// Near-black, near-white and grey pixels are skipped, so a dark poster still gives its accent colour.
pub fn spine_colour(poster: &[u8]) -> Result<String, String> {
    let config = SpineColourConfig {
        fallback: "#3a3a3c",
        darkest: 40,
        lightest: 215,
        least_saturation: 40,
    };
    let decoded =
        image::load_from_memory(poster).map_err(|err| format!("Can't read a poster: {err}"))?;
    // Shrink only: enlarging a small image would blend its colours together.
    let is_large = decoded.width() > 64 || decoded.height() > 96;
    let image = if is_large {
        decoded.thumbnail(64, 96).to_rgb8()
    } else {
        decoded.to_rgb8()
    };

    // Each bucket is a coarse colour (8 levels per channel), with the sums to average it back out.
    let mut buckets: HashMap<(u8, u8, u8), (u32, [u32; 3])> = HashMap::new();
    for pixel in image.pixels() {
        let [red, green, blue] = pixel.0;
        let brightest = red.max(green).max(blue);
        let dimmest = red.min(green).min(blue);
        let is_strong_colour = brightest >= config.darkest
            && dimmest <= config.lightest
            && brightest - dimmest >= config.least_saturation;
        if !is_strong_colour {
            continue;
        }
        let bucket = buckets
            .entry((red >> 5, green >> 5, blue >> 5))
            .or_insert((0, [0, 0, 0]));
        bucket.0 += 1;
        bucket.1[0] += u32::from(red);
        bucket.1[1] += u32::from(green);
        bucket.1[2] += u32::from(blue);
    }

    let Some((count, sums)) = buckets.into_values().max_by_key(|(count, _)| *count) else {
        return Ok(config.fallback.to_string());
    };
    let [red, green, blue] = sums.map(|sum| sum / count);
    Ok(format!("#{red:02x}{green:02x}{blue:02x}"))
}

struct SpineColourConfig {
    fallback: &'static str,
    darkest: u8,
    lightest: u8,
    least_saturation: u8,
}

pub struct DownloadReport {
    pub saved: usize,
    pub failures: Vec<String>,
}

/// Downloads every image in `wanted` that isn't on disk yet, `workers` at a time.
pub fn download_missing(
    tmdb: &Tmdb,
    art_folder: &Path,
    wanted: Vec<ArtImage>,
    workers: usize,
) -> DownloadReport {
    let mut missing = Vec::new();
    for image in wanted {
        let Some(path) = local_path(art_folder, &image) else {
            continue;
        };
        if !path.is_file() {
            missing.push((image, path));
        }
    }

    let chunk_size = missing.len().div_ceil(workers.max(1)).max(1);
    let mut report = DownloadReport {
        saved: 0,
        failures: Vec::new(),
    };
    thread::scope(|scope| {
        let running: Vec<_> = missing
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || download_chunk(tmdb, chunk)))
            .collect();
        for worker in running {
            let Ok(chunk_report) = worker.join() else {
                continue;
            };
            report.saved += chunk_report.saved;
            report.failures.extend(chunk_report.failures);
        }
    });
    report
}

fn download_chunk(tmdb: &Tmdb, images: &[(ArtImage, PathBuf)]) -> DownloadReport {
    let mut report = DownloadReport {
        saved: 0,
        failures: Vec::new(),
    };
    for (image, path) in images {
        let saved = tmdb
            .image(image.size, &image.tmdb_path)
            .map_err(|err| err.to_string())
            .and_then(|bytes| save(path, &bytes));
        match saved {
            Ok(()) => report.saved += 1,
            Err(err) => report.failures.push(err),
        }
    }
    report
}

/// One matched show's art, as the UI gets it. File paths are only set once the image is on disk.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesArt {
    pub series_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_file: Option<String>,
    pub logo_file: Option<String>,
    /// `#rrggbb`, or `None` until the poster has downloaded.
    pub spine_colour: Option<String>,
    pub seasons: Vec<SeasonArt>,
    pub movies: Vec<MovieArt>,
}

/// A series movie. The TMDB parts are empty until it's matched and its art has downloaded.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieArt {
    pub movie_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub poster_file: Option<String>,
    pub logo_file: Option<String>,
    pub spine_colour: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonArt {
    pub number: u32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_file: Option<String>,
}

/// One matched show's art as the database has it, with TMDB image paths.
pub struct SeriesArtRow {
    pub series_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub logo_path: Option<String>,
    pub spine_colour: Option<String>,
    pub seasons: Vec<SeasonArtRow>,
    pub movies: Vec<MovieArtRow>,
}

pub struct MovieArtRow {
    pub movie_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub logo_path: Option<String>,
    pub spine_colour: Option<String>,
}

pub struct SeasonArtRow {
    pub number: u32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

/// Swaps TMDB image paths for files on disk, leaving out images that haven't downloaded.
pub fn series_art(art_folder: &Path, rows: Vec<SeriesArtRow>) -> Vec<SeriesArt> {
    let mut all_art = Vec::new();
    for row in rows {
        let mut seasons = Vec::new();
        for season in row.seasons {
            seasons.push(SeasonArt {
                number: season.number,
                name: season.name,
                overview: season.overview,
                poster_file: season
                    .poster_path
                    .and_then(|path| existing_file(art_folder, ArtImage::poster(path))),
            });
        }
        all_art.push(SeriesArt {
            series_id: row.series_id,
            name: row.name,
            overview: row.overview,
            poster_file: row
                .poster_path
                .and_then(|path| existing_file(art_folder, ArtImage::poster(path))),
            logo_file: row
                .logo_path
                .and_then(|path| existing_file(art_folder, ArtImage::logo(path))),
            spine_colour: row.spine_colour,
            seasons,
            movies: movie_art(art_folder, row.movies),
        });
    }
    all_art
}

fn movie_art(art_folder: &Path, rows: Vec<MovieArtRow>) -> Vec<MovieArt> {
    let mut movies = Vec::new();
    for row in rows {
        movies.push(MovieArt {
            movie_id: row.movie_id,
            title: row.title,
            year: row.year,
            overview: row.overview,
            poster_file: row
                .poster_path
                .and_then(|path| existing_file(art_folder, ArtImage::poster(path))),
            logo_file: row
                .logo_path
                .and_then(|path| existing_file(art_folder, ArtImage::logo(path))),
            spine_colour: row.spine_colour,
        });
    }
    movies
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("fony-art-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    fn png_of(colours: &[[u8; 3]]) -> Vec<u8> {
        let width = u32::try_from(colours.len()).unwrap();
        let mut image = image::RgbImage::new(width, 10);
        for (x, _, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb(colours[usize::try_from(x).unwrap()]);
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn picks_the_most_common_strong_colour_and_skips_black() {
        let black = [5, 5, 5];
        let red = [200, 30, 30];
        let blue = [30, 30, 200];
        let poster = png_of(&[black, black, black, black, black, red, red, red, blue]);
        assert_eq!(spine_colour(&poster).unwrap(), "#c81e1e");
    }

    #[test]
    fn falls_back_to_dark_grey_when_a_poster_has_no_strong_colour() {
        let poster = png_of(&[[0, 0, 0], [128, 128, 128], [250, 250, 250]]);
        assert_eq!(spine_colour(&poster).unwrap(), "#3a3a3c");
        assert!(spine_colour(b"not an image").is_err());
    }

    /// Needs the art downloaded: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn picks_a_colour_from_a_real_poster() {
        let poster = dirs_art_folder().join("w780/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg");
        let colour = spine_colour(&fs::read(poster).unwrap()).unwrap();
        println!("DuckTales spine: {colour}");
        assert_eq!(colour.len(), 7);
    }

    fn dirs_art_folder() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap())
            .join("Library/Application Support/FonyMediaCentre/art")
    }

    #[test]
    fn puts_each_size_in_its_own_folder() {
        let art = Path::new("/art");
        assert_eq!(
            local_path(
                art,
                &ArtImage::poster("/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg".into())
            ),
            Some(PathBuf::from("/art/w780/l39HDf6WYzV8VLY5EnCRC81DWOy.jpg"))
        );
        assert_eq!(
            local_path(
                art,
                &ArtImage::logo("/rx0din6FtPgTy5ZRAI2JuOCH283.png".into())
            ),
            Some(PathBuf::from("/art/w500/rx0din6FtPgTy5ZRAI2JuOCH283.png"))
        );
    }

    #[test]
    fn refuses_anything_but_a_plain_tmdb_file_name() {
        let art = Path::new("/art");
        let unsafe_names = [
            "/../../etc/passwd.jpg",
            "/sub/folder.jpg",
            "abc.jpg",
            "/.jpg",
            "/abc.exe",
            "/abc.jpg.part",
            "/ab c.jpg",
            "",
        ];
        for name in unsafe_names {
            assert_eq!(
                local_path(art, &ArtImage::poster(name.into())),
                None,
                "{name}"
            );
        }
    }

    #[test]
    fn saves_through_a_partial_file_and_reports_existing_files() {
        let folder = temp_folder("save");
        let image = ArtImage::poster("/abc.jpg".into());
        assert_eq!(
            existing_file(&folder, ArtImage::poster("/abc.jpg".into())),
            None
        );

        let path = local_path(&folder, &image).unwrap();
        save(&path, b"jpeg bytes").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"jpeg bytes");
        assert!(!path.with_extension("part").exists());
        assert_eq!(
            existing_file(&folder, image),
            Some(path.to_string_lossy().into_owned())
        );
        fs::remove_dir_all(folder).unwrap();
    }
}

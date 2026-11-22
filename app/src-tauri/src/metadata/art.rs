//! The art folder in Application Support: where each TMDB image lives, and saving downloads safely.
//! A file that exists is never downloaded again, because TMDB never reuses an image's file name.

use super::tmdb::Tmdb;
use serde::Serialize;
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
    pub seasons: Vec<SeasonArt>,
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
    pub seasons: Vec<SeasonArtRow>,
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
            seasons,
        });
    }
    all_art
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

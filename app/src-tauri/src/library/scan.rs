//! Walks one library folder (e.g. MEDIA/Cartoons) and works out every show and file in it.
//! Only reads the drive. The result goes to `db::save_scan`, which does all the writing.

use super::loose::LooseRules;
use super::markers::{self, FolderRule, Mode, SeriesInfo};
use super::parse::{Placement, TidyRules, is_video};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub struct ScanResult {
    pub series: Vec<ScannedSeries>,
    /// Videos sitting loose in the library folder, outside any show.
    pub loose_files: Vec<ScannedFile>,
}

pub struct ScannedSeries {
    /// Relative to the library folder: "DuckTales (1987)", or "Top/Sub" for a kept subfolder.
    pub folder: String,
    pub name: String,
    pub year: Option<i32>,
    pub tmdb_id: Option<i64>,
    pub mode: SeriesMode,
    pub source: SeriesSource,
    pub files: Vec<(ScannedFile, Placement)>,
    /// Some files were only placed by the looser rules, so tapehead hasn't tidied this folder yet.
    pub used_loose_rules: bool,
}

impl ScannedSeries {
    /// Tidy means every file fits the layout. Untidy shows go to the review queue.
    pub fn is_tidy(&self) -> bool {
        !self.used_loose_rules
            && !self
                .files
                .iter()
                .any(|(_, placement)| *placement == Placement::Unplaced)
    }
}

pub struct ScannedFile {
    /// Relative to the drive, e.g. "MEDIA/Cartoons/DuckTales (1987)/Season 01/....mkv".
    pub rel_path: String,
    pub size: i64,
    /// Seconds since 1970, UTC.
    pub modified_secs: i64,
}

#[derive(Clone, Copy)]
pub enum SeriesMode {
    Tmdb,
    Local,
    Keep,
}

impl SeriesMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SeriesMode::Tmdb => "tmdb",
            SeriesMode::Local => "local",
            SeriesMode::Keep => "keep",
        }
    }
}

#[derive(Clone, Copy)]
pub enum SeriesSource {
    Marker,
    SeriesYaml,
    Parsed,
}

impl SeriesSource {
    pub fn as_str(self) -> &'static str {
        match self {
            SeriesSource::Marker => "marker",
            SeriesSource::SeriesYaml => "series_yaml",
            SeriesSource::Parsed => "parsed",
        }
    }
}

/// `library_folder` is the folder on disk; `rel_path` is the same folder relative to its drive.
/// Any read error (such as the drive being unplugged) stops the whole scan, so nothing half-done is saved.
pub fn scan_library_folder(
    library_folder: &Path,
    rel_path: &str,
    rules: &TidyRules,
) -> Result<ScanResult, String> {
    // A broken series.yaml means no folder rules, the same as having none.
    let series_map = markers::read_series_map(library_folder).unwrap_or_default();
    let loose = LooseRules::new();
    let mut result = ScanResult {
        series: Vec::new(),
        loose_files: Vec::new(),
    };

    for entry in sorted_entries(library_folder)? {
        let name = entry_name(&entry)?;
        let path = entry.path();
        if path.is_file() {
            if is_video(&name) {
                result
                    .loose_files
                    .push(scanned_file(&path, &format!("{rel_path}/{name}"))?);
            }
            continue;
        }
        let show = ShowFolder {
            path: &path,
            folder: &name,
            rel_path: &format!("{rel_path}/{name}"),
        };
        let marker = markers::read_marker(&path).ok().flatten();
        match marker {
            Some(info) => result.series.extend(scan_marked_show(&show, info, rules)?),
            None => result
                .series
                .extend(scan_unmarked_show(&show, &series_map, rules, &loose)?),
        }
    }
    Ok(result)
}

struct ShowFolder<'a> {
    path: &'a Path,
    /// Folder name inside the library folder.
    folder: &'a str,
    /// Relative to the drive.
    rel_path: &'a str,
}

fn scan_marked_show(
    show: &ShowFolder,
    info: SeriesInfo,
    rules: &TidyRules,
) -> Result<Option<ScannedSeries>, String> {
    let Some(mode) = series_mode(info.mode) else {
        return Ok(None);
    };
    let placed = placed_files(show.path, show.rel_path, &[], rules, None)?;
    Ok(Some(ScannedSeries {
        folder: show.folder.to_string(),
        name: info.name,
        year: info.year,
        tmdb_id: info.tmdb_id,
        mode,
        source: SeriesSource::Marker,
        files: placed.files,
        used_loose_rules: placed.used_loose_rules,
    }))
}

/// No marker: `series.yaml` may hide the folder or pick out a subfolder to show as its own series.
/// Otherwise the folder name gives the show's name and year.
/// Files that don't fit the tidy layout get a second try with the looser rules.
fn scan_unmarked_show(
    show: &ShowFolder,
    series_map: &[markers::SeriesMapEntry],
    rules: &TidyRules,
    loose: &LooseRules,
) -> Result<Vec<ScannedSeries>, String> {
    let subfolders = subfolder_names(show.path)?;
    let folder_rules = markers::folder_rules(show.folder, &subfolders, series_map);

    let mut series = Vec::new();
    let mut claimed_subfolders = Vec::new();
    let mut rule_for_show: Option<&FolderRule> = None;

    for rule in &folder_rules {
        let Some(subfolder) = &rule.subfolder else {
            rule_for_show = rule_for_show.or(Some(rule));
            continue;
        };
        claimed_subfolders.push(subfolder.clone());
        let Some(mode) = series_mode(rule.info.mode) else {
            continue;
        };
        let placed = placed_files(
            &show.path.join(subfolder),
            &format!("{}/{subfolder}", show.rel_path),
            &[],
            rules,
            Some(loose),
        )?;
        series.push(ScannedSeries {
            folder: format!("{}/{subfolder}", show.folder),
            name: rule.info.name.clone(),
            year: rule.info.year,
            tmdb_id: rule.info.tmdb_id,
            mode,
            source: SeriesSource::SeriesYaml,
            files: placed.files,
            used_loose_rules: placed.used_loose_rules,
        });
    }

    let PlacedFiles {
        files,
        used_loose_rules,
    } = placed_files(
        show.path,
        show.rel_path,
        &claimed_subfolders,
        rules,
        Some(loose),
    )?;
    if files.is_empty() {
        return Ok(series);
    }

    let show_series = match rule_for_show {
        Some(rule) => series_mode(rule.info.mode).map(|mode| ScannedSeries {
            folder: show.folder.to_string(),
            name: rule.info.name.clone(),
            year: rule.info.year,
            tmdb_id: rule.info.tmdb_id,
            mode,
            source: SeriesSource::SeriesYaml,
            files,
            used_loose_rules,
        }),
        None => {
            let (name, year) = loose.show_name_and_year(show.folder);
            Some(ScannedSeries {
                folder: show.folder.to_string(),
                name,
                year,
                tmdb_id: None,
                mode: SeriesMode::Local,
                source: SeriesSource::Parsed,
                files,
                used_loose_rules,
            })
        }
    };
    series.extend(show_series);
    Ok(series)
}

/// `None` means hidden.
fn series_mode(mode: Mode) -> Option<SeriesMode> {
    match mode {
        Mode::Tmdb => Some(SeriesMode::Tmdb),
        Mode::Local => Some(SeriesMode::Local),
        Mode::Keep => Some(SeriesMode::Keep),
        Mode::Skip => None,
    }
}

struct PlacedFiles {
    files: Vec<(ScannedFile, Placement)>,
    used_loose_rules: bool,
}

/// Every video under `folder`, placed by the tidy rules, skipping the top-level subfolders in `skip`.
/// With `loose`, files the tidy rules can't place get a second try with the looser ones.
fn placed_files(
    folder: &Path,
    folder_rel_path: &str,
    skip: &[String],
    rules: &TidyRules,
    loose: Option<&LooseRules>,
) -> Result<PlacedFiles, String> {
    let mut videos = Vec::new();
    collect_videos(folder, "", skip, &mut videos)?;

    let mut files = Vec::new();
    let mut untidy_paths = Vec::new();
    for (path, path_in_show) in &videos {
        let file = scanned_file(path, &format!("{folder_rel_path}/{path_in_show}"))?;
        let placement = rules.place_file(path_in_show);
        if placement == Placement::Unplaced {
            untidy_paths.push((files.len(), path_in_show.as_str()));
        }
        files.push((file, placement));
    }

    let Some(loose) = loose else {
        return Ok(PlacedFiles {
            files,
            used_loose_rules: false,
        });
    };
    let paths: Vec<&str> = untidy_paths.iter().map(|(_, path)| *path).collect();
    let mut used_loose_rules = false;
    for ((index, _), placement) in untidy_paths.iter().zip(loose.place_files(&paths)) {
        if placement != Placement::Unplaced {
            used_loose_rules = true;
        }
        files[*index].1 = placement;
    }
    Ok(PlacedFiles {
        files,
        used_loose_rules,
    })
}

/// Depth-first walk. `prefix` is the path so far inside the show, e.g. "Season 01/".
fn collect_videos(
    folder: &Path,
    prefix: &str,
    skip: &[String],
    videos: &mut Vec<(std::path::PathBuf, String)>,
) -> Result<(), String> {
    for entry in sorted_entries(folder)? {
        let name = entry_name(&entry)?;
        let path = entry.path();
        if path.is_dir() {
            if prefix.is_empty() && skip.contains(&name) {
                continue;
            }
            collect_videos(&path, &format!("{prefix}{name}/"), &[], videos)?;
            continue;
        }
        if is_video(&name) {
            videos.push((path, format!("{prefix}{name}")));
        }
    }
    Ok(())
}

fn subfolder_names(folder: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in sorted_entries(folder)? {
        if entry.path().is_dir() {
            names.push(entry_name(&entry)?);
        }
    }
    Ok(names)
}

/// A folder's entries in name order, without hidden ones (`.tapehead`, `.DS_Store`, `._` copies).
fn sorted_entries(folder: &Path) -> Result<Vec<fs::DirEntry>, String> {
    let reader =
        fs::read_dir(folder).map_err(|err| format!("Can't read {}: {err}", folder.display()))?;
    let mut entries = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|err| format!("Can't read {}: {err}", folder.display()))?;
        if !entry.file_name().to_string_lossy().starts_with('.') {
            entries.push(entry);
        }
    }
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

fn entry_name(entry: &fs::DirEntry) -> Result<String, String> {
    entry
        .file_name()
        .into_string()
        .map_err(|name| format!("A file name isn't valid text: {}", name.to_string_lossy()))
}

fn scanned_file(path: &Path, rel_path: &str) -> Result<ScannedFile, String> {
    let metadata =
        fs::metadata(path).map_err(|err| format!("Can't read {}: {err}", path.display()))?;
    let modified_secs = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|age| i64::try_from(age.as_secs()).ok())
        .unwrap_or(0);
    Ok(ScannedFile {
        rel_path: rel_path.to_string(),
        size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        modified_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the media drive plugged in: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scans_the_real_cartoons_folder() {
        let cartoons = Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons");
        let result = scan_library_folder(cartoons, "MEDIA/Cartoons", &TidyRules::new()).unwrap();

        let mut placed = 0;
        let mut unplaced = 0;
        for series in &result.series {
            for (_, placement) in &series.files {
                if *placement == Placement::Unplaced {
                    unplaced += 1;
                    continue;
                }
                placed += 1;
            }
        }
        println!(
            "{} shows, {placed} placed, {unplaced} unplaced, {} loose",
            result.series.len(),
            result.loose_files.len()
        );

        let folders: Vec<&str> = result
            .series
            .iter()
            .map(|series| series.folder.as_str())
            .collect();
        assert!(folders.contains(
            &"The Pink Panther Cartoon Collection/The BEST of the Pink Panther Collection"
        ));
        assert!(!folders.contains(&"The Pink Panther Cartoon Collection"));
        assert!(folders.contains(&"Dungeons & Dragons (1983)"));
        assert_eq!(result.series.len(), 30);
        assert!(placed > 2600);
        // Only the two Private Snafu shorts inside the Looney Tunes folder.
        assert_eq!(unplaced, 2);
        assert!(result.series.iter().any(|series| series.name
            == "He-Man and the Masters of the Universe"
            && series.year == Some(1983)));
    }
}

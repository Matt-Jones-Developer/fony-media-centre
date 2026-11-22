//! Reads the YAML files tapehead leaves on the drive. Read only.
//! - `.tapehead-series.yaml` in a show folder: written after tapehead tidies that show.
//! - `Cartoons/.tapehead/series.yaml`: tapehead's map of every show, used for folders with no marker.

use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// What tapehead knows about one show.
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SeriesInfo {
    pub name: String,
    pub year: Option<i32>,
    pub mode: Mode,
    pub tmdb_id: Option<i64>,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Tidied with TMDB's episode list.
    Tmdb,
    /// Tidied with the file's own numbering (Dexter's Laboratory).
    Local,
    /// Shown as-is, never renamed (the BEST of the Pink Panther).
    Keep,
    /// Hidden from the library.
    Skip,
}

/// One entry in `series.yaml`. Its key is `<top folder>::<tapehead's short name>`.
#[derive(Debug, Deserialize)]
pub struct SeriesMapEntry {
    pub key: String,
    #[serde(flatten)]
    pub info: SeriesInfo,
}

#[derive(Deserialize)]
struct SeriesMap {
    series: Vec<SeriesMapEntry>,
}

/// What `series.yaml` says about a folder: the show folder itself (`subfolder: None`) or one inside it.
#[derive(Debug, PartialEq)]
pub struct FolderRule {
    pub subfolder: Option<String>,
    pub info: SeriesInfo,
}

/// `Ok(None)` when the show has no marker. `Err` when there is one but it can't be read.
pub fn read_marker(show_folder: &Path) -> Result<Option<SeriesInfo>, String> {
    let Some(text) = read_if_present(&show_folder.join(".tapehead-series.yaml"))? else {
        return Ok(None);
    };
    parse_marker(&text).map(Some)
}

/// `Ok(empty)` when there's no `series.yaml`.
pub fn read_series_map(library_folder: &Path) -> Result<Vec<SeriesMapEntry>, String> {
    let Some(text) = read_if_present(&library_folder.join(".tapehead/series.yaml"))? else {
        return Ok(Vec::new());
    };
    parse_series_map(&text)
}

pub fn parse_marker(text: &str) -> Result<SeriesInfo, String> {
    serde_norway::from_str(text).map_err(|err| format!("Marker can't be read: {err}"))
}

pub fn parse_series_map(text: &str) -> Result<Vec<SeriesMapEntry>, String> {
    let map: SeriesMap =
        serde_norway::from_str(text).map_err(|err| format!("series.yaml can't be read: {err}"))?;
    Ok(map.series)
}

/// The `series.yaml` rules for one unmarked show folder.
/// An entry applies to the first subfolder whose name matches its short name, otherwise to the folder itself.
pub fn folder_rules(
    top_folder: &str,
    subfolders: &[String],
    entries: &[SeriesMapEntry],
) -> Vec<FolderRule> {
    let mut rules = Vec::new();
    for entry in entries {
        let Some((entry_top, short_name)) = entry.key.split_once("::") else {
            continue;
        };
        if entry_top != top_folder {
            continue;
        }
        // tapehead's short names can carry a year after a dash, e.g. "raccoons-1985".
        let wanted: String = short_name
            .chars()
            .filter(|char| char.is_ascii_alphanumeric())
            .collect();
        let subfolder = subfolders
            .iter()
            .find(|folder| short_name_of(folder).starts_with(&wanted))
            .cloned();
        rules.push(FolderRule {
            subfolder,
            info: entry.info.clone(),
        });
    }
    rules
}

/// tapehead's `norm()`: "The Ren & Stimpy Show" -> "renandstimpyshow". Keep in step with
/// tools/tapehead/src/tapehead/survey.py, or `series.yaml` entries stop finding their folders.
fn short_name_of(name: &str) -> String {
    // "n'", "'n'" and a lone "n" all mean "and" (Chip 'n' Dale, Rock n Roll).
    let and_words = Regex::new(r"\bn'|'n'|\bn\b").expect("valid pattern");
    let leading_the = Regex::new(r"^the\s+").expect("valid pattern");

    let lower = name.to_lowercase().replace('&', "and");
    let with_and = and_words.replace_all(&lower, "and");
    let without_the = leading_the.replace(&with_and, "");
    without_the
        .chars()
        .filter(|char| char.is_ascii_alphanumeric())
        .collect()
}

fn read_if_present(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("Can't read {}: {err}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_marker() {
        let text = "# Written by tapehead after tidying this show.\nname: Challenge of the Gobots\nyear: 1984\nmode: tmdb\ntmdb_id: 697\ntmdb_name: Challenge of the GoBots\n";
        assert_eq!(
            parse_marker(text).unwrap(),
            SeriesInfo {
                name: "Challenge of the Gobots".into(),
                year: Some(1984),
                mode: Mode::Tmdb,
                tmdb_id: Some(697),
            }
        );
    }

    #[test]
    fn reads_a_marker_with_no_tmdb_id() {
        let text =
            "name: Dexter's Laboratory\nyear: 1996\nmode: local\ntmdb_id: null\ntmdb_name: null\n";
        let info = parse_marker(text).unwrap();
        assert_eq!((info.mode, info.tmdb_id), (Mode::Local, None));
    }

    #[test]
    fn refuses_a_broken_marker() {
        assert!(parse_marker("name: [unclosed").is_err());
        assert!(parse_marker("name: Show\nmode: sideways\n").is_err());
        assert!(parse_marker("year: 1984\nmode: tmdb\n").is_err());
    }

    #[test]
    fn makes_the_same_short_names_as_tapehead() {
        assert_eq!(short_name_of("The Ren & Stimpy Show"), "renandstimpyshow");
        assert_eq!(
            short_name_of("Chip 'n' Dale Rescue Rangers (1989)"),
            "chipanddalerescuerangers1989"
        );
        assert_eq!(
            short_name_of("The BEST of the Pink Panther Collection"),
            "bestofthepinkpanthercollection"
        );
        assert_eq!(short_name_of("M.A.S.K. (1985)"), "mask1985");
    }

    #[test]
    fn hides_the_pink_panther_collection_but_keeps_its_best_of_folder() {
        let yaml = "series:
- key: The Pink Panther Cartoon Collection::bestofthepinkpanther
  name: The Pink Panther
  year: 1964
  mode: keep
  tmdb_id: null
  tmdb_name: null
  force_season: null
  files: 28
  sample: 004 Dial 'P' For Pink (Mar 17, 1965).mp4
  patterns:
    absolute: 28
- key: The Pink Panther Cartoon Collection::pinkpanthercartoon
  name: The Pink Panther
  year: 1964
  mode: skip
  tmdb_id: null
  tmdb_name: null
- key: The Raccoons (1985)::raccoons-1985
  name: The Raccoons
  year: 1985
  mode: tmdb
  tmdb_id: 2513
";
        let entries = parse_series_map(yaml).unwrap();
        let rules = folder_rules(
            "The Pink Panther Cartoon Collection",
            &["The BEST of the Pink Panther Collection".to_string()],
            &entries,
        );

        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0].subfolder.as_deref(),
            Some("The BEST of the Pink Panther Collection")
        );
        assert_eq!(rules[0].info.mode, Mode::Keep);
        assert_eq!(rules[1].subfolder, None);
        assert_eq!(rules[1].info.mode, Mode::Skip);
    }

    #[test]
    fn a_folder_with_no_entries_has_no_rules() {
        let entries = parse_series_map("series: []\n").unwrap();
        assert!(folder_rules("Looney Tunes - Golden Collection", &[], &entries).is_empty());
    }

    /// Needs the media drive plugged in: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn reads_the_real_tapehead_files() {
        let cartoons = Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons");
        let entries = read_series_map(cartoons).unwrap();
        assert!(!entries.is_empty());
        let marker = read_marker(&cartoons.join("DuckTales (1987)")).unwrap();
        assert_eq!(marker.map(|info| info.name), Some("DuckTales".to_string()));
        assert_eq!(
            read_marker(&cartoons.join("Looney Tunes - Golden Collection")).unwrap(),
            None
        );
    }
}

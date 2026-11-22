//! Looser name rules for show folders tapehead hasn't tidied (no `.tapehead-series.yaml`).
//! Ported from tapehead's `parse.py`, but only the patterns these folders actually use.
//! Like `parse.rs`, no file access here, so every rule can be tested with plain strings.

use super::parse::{Episode, Placement};
use regex::Regex;
use std::path::Path;

/// Build once per scan and reuse: compiling the patterns is the slow part.
pub struct LooseRules {
    episode: Regex,
    season_in_folder: Regex,
    numbered: Regex,
    dated_short: Regex,
    year_folder: Regex,
    year_in_name: Regex,
    season_range_in_name: Regex,
    collection_in_name: Regex,
}

/// A short whose episode number depends on the other shorts in its decade.
struct DatedShort {
    index: usize,
    decade: u32,
    /// Year, month, day: sorts into release order.
    released: (u32, u32, u32),
    title: String,
}

impl LooseRules {
    pub fn new() -> Self {
        LooseRules {
            // "He-Man S01E01 Diamond Ray of Disappearance"
            episode: Regex::new(
                r"(?i)\bS(?P<season>\d{1,2})\s*E(?P<episode>\d{1,3})(?P<segment>[ab])?\b\s*(?:-\s*)?(?P<title>.*)$",
            )
            .expect("valid loose episode pattern"),
            // "He-Man and the Masters of the Universe S01 (360p re-dvdrip)" or "Season 1"
            season_in_folder: Regex::new(r"(?i)(?:^|[\s._-])(?:S|Season\s*)(?P<season>\d{1,2})\b")
                .expect("valid season folder pattern"),
            // "004 Dial 'P' For Pink (Mar 17, 1965)"
            numbered: Regex::new(r"^(?P<number>\d{3}) (?P<title>.+?)(?: \([A-Za-z]{3} \d{1,2}, \d{4}\))?$")
                .expect("valid numbered pattern"),
            // "090030DVD Congo Jazz LT": released 09/00/30. LT, MM and BR are disc tags, not title.
            dated_short: Regex::new(
                r"^(?P<month>\d{2})(?P<day>\d{2})(?P<year>\d{2})(?:DVD)? (?P<title>.+?)(?: (?:LT|MM))?(?: BR)?$",
            )
            .expect("valid dated short pattern"),
            year_folder: Regex::new(r"^(?P<year>19\d{2})$").expect("valid year folder pattern"),
            // "(1983)", "(1983-)" or "(1983-1985)"
            year_in_name: Regex::new(r"\((?P<year>(?:19|20)\d{2})(?:-(?:\d{4})?)?\)")
                .expect("valid year in name pattern"),
            // "S01-S02" or "S01"
            season_range_in_name: Regex::new(r"(?i)\bS\d{1,2}(?:-S\d{1,2})?\b")
                .expect("valid season range pattern"),
            // " - Golden Collection"
            collection_in_name: Regex::new(r"(?i) - [^-]*\bCollection$")
                .expect("valid collection pattern"),
        }
    }

    /// Places every file in one show folder. `paths_in_show` are relative to the show folder.
    /// Shorts are numbered within their decade, which is why this takes the whole folder at once.
    pub fn place_files(&self, paths_in_show: &[&str]) -> Vec<Placement> {
        let mut placements = Vec::new();
        let mut shorts = Vec::new();
        for (index, path) in paths_in_show.iter().enumerate() {
            if let Some(short) = self.dated_short(index, path) {
                shorts.push(short);
                placements.push(Placement::Unplaced);
                continue;
            }
            placements.push(self.place_file(path));
        }
        number_shorts(shorts, &mut placements);
        placements
    }

    fn place_file(&self, path_in_show: &str) -> Placement {
        let parts: Vec<&str> = path_in_show.split('/').collect();
        let Some((file_name, folders)) = parts.split_last() else {
            return Placement::Unplaced;
        };
        let stem = file_stem(file_name);

        if folders
            .first()
            .is_some_and(|folder| folder.eq_ignore_ascii_case("Extras"))
        {
            return Placement::Extra {
                title: stem.to_string(),
            };
        }
        match folders {
            [] => self.place_numbered(stem),
            [folder] => self.place_episode(stem, folder),
            _ => Placement::Unplaced,
        }
    }

    /// "Show S01E02 Title" inside a folder whose name says the same season.
    fn place_episode(&self, stem: &str, folder: &str) -> Placement {
        let Some(found) = self.episode.captures(stem) else {
            return Placement::Unplaced;
        };
        let Some(folder_season) = self.season_from_folder(folder) else {
            return Placement::Unplaced;
        };
        let (Ok(season), Ok(episode)) = (found["season"].parse(), found["episode"].parse()) else {
            return Placement::Unplaced;
        };
        if season != folder_season {
            return Placement::Unplaced;
        }
        Placement::Episode(Episode {
            season,
            episode,
            episode_end: None,
            segment: found
                .name("segment")
                .and_then(|segment| segment.as_str().to_ascii_lowercase().chars().next()),
            title: non_empty(&found["title"]),
        })
    }

    fn season_from_folder(&self, folder: &str) -> Option<u32> {
        let found = self.season_in_folder.captures(folder)?;
        found["season"].parse().ok()
    }

    /// "004 Title (Mar 17, 1965)" loose in the show folder: season 1, in file name order.
    fn place_numbered(&self, stem: &str) -> Placement {
        let Some(found) = self.numbered.captures(stem) else {
            return Placement::Unplaced;
        };
        let Ok(episode) = found["number"].parse() else {
            return Placement::Unplaced;
        };
        Placement::Episode(Episode {
            season: 1,
            episode,
            episode_end: None,
            segment: None,
            title: non_empty(&found["title"]),
        })
    }

    /// "1930/090030DVD Congo Jazz LT.avi". The folder year must agree with the file's date.
    fn dated_short(&self, index: usize, path_in_show: &str) -> Option<DatedShort> {
        let (folder, file_name) = path_in_show.split_once('/')?;
        let year: u32 = self.year_folder.captures(folder)?["year"].parse().ok()?;
        let found = self.dated_short.captures(file_stem(file_name))?;
        let short_year: u32 = found["year"].parse().ok()?;
        if short_year != year % 100 {
            return None;
        }
        // Day 00 is real: some release dates only give the month.
        let month: u32 = found["month"].parse().ok()?;
        let day: u32 = found["day"].parse().ok()?;
        if !(1..=12).contains(&month) || day > 31 {
            return None;
        }
        Some(DatedShort {
            index,
            decade: year / 10 * 10,
            released: (year, month, day),
            title: found["title"].to_string(),
        })
    }

    /// "He-Man and the Masters of the Universe S01-S02 (1983-)" to the name and year.
    pub fn show_name_and_year(&self, folder: &str) -> (String, Option<i32>) {
        let year = self
            .year_in_name
            .captures(folder)
            .and_then(|found| found["year"].parse().ok());
        let name = self.year_in_name.replace_all(folder, "");
        let name = self.season_range_in_name.replace_all(&name, "");
        let name = self.collection_in_name.replace(name.trim(), "");
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        (name, year)
    }
}

/// Each decade is one season (numbered by its first year, e.g. 1930), shorts in release order.
fn number_shorts(mut shorts: Vec<DatedShort>, placements: &mut [Placement]) {
    shorts.sort_by(|a, b| (a.decade, a.released, &a.title).cmp(&(b.decade, b.released, &b.title)));
    let mut current_decade = None;
    let mut episode = 0;
    for short in shorts {
        if current_decade != Some(short.decade) {
            current_decade = Some(short.decade);
            episode = 0;
        }
        episode += 1;
        placements[short.index] = Placement::Episode(Episode {
            season: short.decade,
            episode,
            episode_end: None,
            segment: None,
            title: Some(short.title),
        });
    }
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

fn file_stem(file_name: &str) -> &str {
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(path: &str) -> Placement {
        LooseRules::new().place_files(&[path]).remove(0)
    }

    fn episode(path: &str) -> Episode {
        match place(path) {
            Placement::Episode(episode) => episode,
            other => panic!("{path} should be an episode, got {other:?}"),
        }
    }

    #[test]
    fn reads_he_man_episodes_with_the_season_from_the_folder() {
        let found = episode(
            "He-Man and the Masters of the Universe S02 (360p re-dvdrip)/He-Man S02E15 The Arena.mp4",
        );
        assert_eq!((found.season, found.episode), (2, 15));
        assert_eq!(found.title.as_deref(), Some("The Arena"));
    }

    #[test]
    fn leaves_an_episode_in_the_wrong_season_folder_unplaced() {
        assert_eq!(
            place("He-Man S01 (360p re-dvdrip)/He-Man S02E15 The Arena.mp4"),
            Placement::Unplaced
        );
        assert_eq!(
            place("Disc 1/He-Man S02E15 The Arena.mp4"),
            Placement::Unplaced
        );
    }

    #[test]
    fn reads_numbered_shorts_and_drops_the_date() {
        let found = episode("004 Dial 'P' For Pink (Mar 17, 1965).mp4");
        assert_eq!((found.season, found.episode), (1, 4));
        assert_eq!(found.title.as_deref(), Some("Dial 'P' For Pink"));
        assert_eq!(
            episode("035 The Hand Is Pinker Than The Eye (Dec 20, 1967).mp4").episode,
            35
        );
    }

    #[test]
    fn groups_dated_shorts_by_decade_in_release_order() {
        let placements = LooseRules::new().place_files(&[
            "1949/062549DVD Long Haired Hare LT.avi",
            "1941/032941DVD Goofy Groceries MM.avi",
            "1930/110030DVD The Booze Hangs High LT.avi",
            "1930/090030DVD Congo Jazz LT.avi",
            "1943/082143DVD Scrap Happy Daffy LT.avi",
        ]);
        let summary: Vec<(u32, u32, String)> = placements
            .into_iter()
            .map(|placement| match placement {
                Placement::Episode(found) => {
                    (found.season, found.episode, found.title.unwrap_or_default())
                }
                other => panic!("expected an episode, got {other:?}"),
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (1940, 3, "Long Haired Hare".into()),
                (1940, 1, "Goofy Groceries".into()),
                (1930, 2, "The Booze Hangs High".into()),
                (1930, 1, "Congo Jazz".into()),
                (1940, 2, "Scrap Happy Daffy".into()),
            ]
        );
    }

    #[test]
    fn strips_disc_tags_from_short_titles() {
        assert_eq!(
            episode("1950/012150DVD Nurdy-Gurdy Hare MM BR.avi")
                .title
                .as_deref(),
            Some("Nurdy-Gurdy Hare")
        );
        assert_eq!(
            episode("1968/020368DVD Norman Normal.avi").title.as_deref(),
            Some("Norman Normal")
        );
    }

    #[test]
    fn leaves_shorts_that_disagree_with_their_folder_unplaced() {
        assert_eq!(
            place("1931/090030DVD Congo Jazz LT.avi"),
            Placement::Unplaced
        );
        assert_eq!(
            place("Private Snafu/060043 Coming Snafu.avi"),
            Placement::Unplaced
        );
        assert_eq!(
            place("1930/130030DVD Bad Month LT.avi"),
            Placement::Unplaced
        );
    }

    #[test]
    fn reads_extras_at_any_depth() {
        assert_eq!(
            place("Extras/Volume 6/Disc 3/The World of Leon Schlesinger/Introduction.avi"),
            Placement::Extra {
                title: "Introduction".into()
            }
        );
    }

    #[test]
    fn cleans_show_folder_names() {
        let rules = LooseRules::new();
        assert_eq!(
            rules.show_name_and_year("He-Man and the Masters of the Universe S01-S02 (1983-)"),
            (
                "He-Man and the Masters of the Universe".to_string(),
                Some(1983)
            )
        );
        assert_eq!(
            rules.show_name_and_year("Looney Tunes - Golden Collection"),
            ("Looney Tunes".to_string(), None)
        );
        assert_eq!(
            rules.show_name_and_year("Dungeons & Dragons (1983)"),
            ("Dungeons & Dragons".to_string(), Some(1983))
        );
        assert_eq!(
            rules.show_name_and_year("Spider-Man and His Amazing Friends (1981)"),
            ("Spider-Man and His Amazing Friends".to_string(), Some(1981))
        );
    }
}

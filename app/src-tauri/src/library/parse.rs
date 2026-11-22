//! Works out where a file belongs inside a tidy show folder, from its path alone.
//! The layout is the one tapehead writes (see CLAUDE.md "The library on disk").
//! No file access here, so every rule can be tested with plain strings.

use regex::Regex;
use std::path::Path;

/// Where a file sits in its show.
#[derive(Debug, PartialEq)]
pub enum Placement {
    Episode(Episode),
    Movie {
        title: String,
        year: Option<i32>,
    },
    Extra {
        title: String,
    },
    /// Doesn't fit the tidy layout. It goes to the review queue.
    Unplaced,
}

#[derive(Debug, PartialEq)]
pub struct Episode {
    /// 0 is Specials.
    pub season: u32,
    pub episode: u32,
    /// Only set for a two-story file such as `S01E03-E04`.
    pub episode_end: Option<u32>,
    /// `a` or `b` for a half-episode such as `S01E01a`.
    pub segment: Option<char>,
    pub title: Option<String>,
}

/// Build once per scan and reuse: compiling the patterns is the slow part.
pub struct TidyRules {
    episode: Regex,
    name_with_year: Regex,
    season_folder: Regex,
}

impl TidyRules {
    pub fn new() -> Self {
        TidyRules {
            // "Show (Year) - S01E03a-E04 - Title". The show name is lazy so the first code wins.
            episode: Regex::new(
                r"^.+? - S(?P<season>\d{2})E(?P<episode>\d{2,3})(?P<segment>[ab])?(?:-E(?P<end>\d{2,3}))?(?: - (?P<title>.+))?$",
            )
            .expect("valid episode pattern"),
            // "Title (1986)"
            name_with_year: Regex::new(r"^(?P<name>.+) \((?P<year>\d{4})\)$")
                .expect("valid name and year pattern"),
            // "Season 01"
            season_folder: Regex::new(r"^Season (?P<season>\d{2})$").expect("valid season pattern"),
        }
    }

    /// `path_in_show` is relative to the show folder, e.g. `Season 01/Show (1987) - S01E03 - Title.mkv`.
    pub fn place_file(&self, path_in_show: &str) -> Placement {
        let parts: Vec<&str> = path_in_show.split('/').collect();
        let [folder, file_name] = parts.as_slice() else {
            return Placement::Unplaced;
        };
        let stem = file_stem(file_name);

        if *folder == "Movies" {
            return self.place_movie(stem);
        }
        if *folder == "Extras" {
            return Placement::Extra {
                title: stem.to_string(),
            };
        }
        if *folder == "Specials" {
            return self.place_episode(stem, 0);
        }
        let Some(folder_season) = self.season_from_folder(folder) else {
            return Placement::Unplaced;
        };
        self.place_episode(stem, folder_season)
    }

    fn season_from_folder(&self, folder: &str) -> Option<u32> {
        let found = self.season_folder.captures(folder)?;
        found["season"].parse().ok()
    }

    fn place_episode(&self, stem: &str, folder_season: u32) -> Placement {
        let Some(found) = self.episode.captures(stem) else {
            return Placement::Unplaced;
        };
        let (Ok(season), Ok(episode)) = (found["season"].parse(), found["episode"].parse()) else {
            return Placement::Unplaced;
        };
        // A file in the wrong season folder is a filing mistake, so let Glitchy check it.
        if season != folder_season {
            return Placement::Unplaced;
        }
        let episode_end: Option<u32> = found.name("end").and_then(|end| end.as_str().parse().ok());
        if episode_end.is_some_and(|end| end <= episode) {
            return Placement::Unplaced;
        }
        Placement::Episode(Episode {
            season,
            episode,
            episode_end,
            segment: found
                .name("segment")
                .and_then(|segment| segment.as_str().chars().next()),
            title: found.name("title").map(|title| title.as_str().to_string()),
        })
    }

    fn place_movie(&self, stem: &str) -> Placement {
        let (title, year) = self.name_and_year(stem);
        Placement::Movie { title, year }
    }

    /// Splits "DuckTales (1987)" into its name and year. With no year, the whole text is the name.
    pub fn name_and_year(&self, text: &str) -> (String, Option<i32>) {
        let Some(found) = self.name_with_year.captures(text) else {
            return (text.to_string(), None);
        };
        (found["name"].to_string(), found["year"].parse().ok())
    }
}

/// Video files the library reads. Hidden files (macOS `._` copies, `.DS_Store`) never count.
pub fn is_video(file_name: &str) -> bool {
    let extensions = [
        "mkv", "mp4", "avi", "m4v", "mov", "mpg", "mpeg", "wmv", "ts", "m2ts", "webm",
    ];
    if file_name.starts_with('.') {
        return false;
    }
    let Some(extension) = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
    else {
        return false;
    };
    extensions.contains(&extension.to_lowercase().as_str())
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

    fn episode(path: &str) -> Episode {
        match TidyRules::new().place_file(path) {
            Placement::Episode(episode) => episode,
            other => panic!("{path} should be an episode, got {other:?}"),
        }
    }

    fn place(path: &str) -> Placement {
        TidyRules::new().place_file(path)
    }

    #[test]
    fn reads_a_plain_episode() {
        let found = episode("Season 01/DuckTales (1987) - S01E03 - Don't Give Up the Ship.mkv");
        assert_eq!(found.season, 1);
        assert_eq!(found.episode, 3);
        assert_eq!(found.episode_end, None);
        assert_eq!(found.segment, None);
        assert_eq!(found.title.as_deref(), Some("Don't Give Up the Ship"));
    }

    #[test]
    fn reads_a_two_story_range() {
        let found = episode(
            "Season 02/Hey Arnold! (1996) - S02E03-E04 - Arnold's Christmas & Part Two.mp4",
        );
        assert_eq!((found.episode, found.episode_end), (3, Some(4)));
        assert_eq!(
            found.title.as_deref(),
            Some("Arnold's Christmas & Part Two")
        );
    }

    #[test]
    fn reads_a_half_episode_segment() {
        let found =
            episode("Season 01/The Ren & Stimpy Show (1991) - S01E01a - Stimpy's Big Day.mkv");
        assert_eq!((found.episode, found.segment), (1, Some('a')));
    }

    #[test]
    fn reads_specials_as_season_zero() {
        let found = episode("Specials/South Park (1997) - S00E01 - The Spirit of Christmas.mkv");
        assert_eq!((found.season, found.episode), (0, 1));
    }

    #[test]
    fn keeps_a_dash_inside_the_title() {
        let found = episode("Season 01/Futurama (1999) - S01E01 - Space Pilot 3000 - Part 1.mkv");
        assert_eq!(found.title.as_deref(), Some("Space Pilot 3000 - Part 1"));
    }

    #[test]
    fn reads_an_episode_with_no_title_and_three_digit_numbers() {
        let found = episode("Season 01/Challenge of the Gobots (1984) - S01E065.mp4");
        assert_eq!(found.episode, 65);
        assert_eq!(found.title, None);
    }

    #[test]
    fn reads_series_movies_with_and_without_a_year() {
        assert_eq!(
            place("Movies/The Transformers The Movie (1986).mp4"),
            Placement::Movie {
                title: "The Transformers The Movie".into(),
                year: Some(1986)
            }
        );
        assert_eq!(
            place("Movies/Pinky and the Brain Special.mkv"),
            Placement::Movie {
                title: "Pinky and the Brain Special".into(),
                year: None
            }
        );
    }

    #[test]
    fn splits_a_folder_name_into_name_and_year() {
        let rules = TidyRules::new();
        assert_eq!(
            rules.name_and_year("Dungeons & Dragons (1983)"),
            ("Dungeons & Dragons".to_string(), Some(1983))
        );
        assert_eq!(
            rules.name_and_year("He-Man and the Masters of the Universe S01-S02 (1983-)"),
            (
                "He-Man and the Masters of the Universe S01-S02 (1983-)".to_string(),
                None
            )
        );
    }

    #[test]
    fn reads_extras() {
        assert_eq!(
            place("Extras/Pencil Tests.mkv"),
            Placement::Extra {
                title: "Pencil Tests".into()
            }
        );
    }

    #[test]
    fn leaves_anything_untidy_unplaced() {
        let untidy = [
            "Season 01/DuckTales (1987) - S02E01 - Wrong Folder.mkv",
            "Season 01/DuckTales (1987) - S01E04-E03 - Backwards Range.mkv",
            "Season 01/01 Battle for Gobotron.mp4",
            "Season 1/DuckTales (1987) - S01E01 - Folder Not Padded.mkv",
            "Specials/DuckTales (1987) - S01E01 - Season One In Specials.mkv",
            "Loose File.mkv",
            "Season 01/Disc 1/DuckTales (1987) - S01E01 - Too Deep.mkv",
            "Bonus/Something.mkv",
        ];
        for path in untidy {
            assert_eq!(place(path), Placement::Unplaced, "{path}");
        }
    }

    /// Needs the media drive plugged in: `cargo test -- --ignored --nocapture`.
    /// Every video in a folder tapehead has tidied should be placed.
    #[test]
    #[ignore]
    fn places_every_file_in_the_tidied_folders_on_the_drive() {
        let cartoons = Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons");
        let rules = TidyRules::new();
        let mut placed = 0;
        let mut unplaced = Vec::new();
        for show in std::fs::read_dir(cartoons).unwrap().flatten() {
            if !show.path().join(".tapehead-series.yaml").exists() {
                continue;
            }
            for folder in std::fs::read_dir(show.path()).unwrap().flatten() {
                let Ok(files) = std::fs::read_dir(folder.path()) else {
                    continue;
                };
                for file in files.flatten() {
                    let file_name = file.file_name().to_string_lossy().to_string();
                    if !is_video(&file_name) {
                        continue;
                    }
                    let path = format!("{}/{file_name}", folder.file_name().to_string_lossy());
                    if rules.place_file(&path) == Placement::Unplaced {
                        unplaced.push(path);
                        continue;
                    }
                    placed += 1;
                }
            }
        }
        println!("{placed} placed");
        assert!(unplaced.is_empty(), "unplaced: {unplaced:#?}");
        assert!(placed > 2000);
    }

    #[test]
    fn knows_which_files_are_video() {
        assert!(is_video("Show - S01E01.mkv"));
        assert!(is_video("Show - S01E01.MP4"));
        assert!(!is_video("._Show - S01E01.mkv"));
        assert!(!is_video(".DS_Store"));
        assert!(!is_video("notes.txt"));
        assert!(!is_video("no extension"));
    }
}

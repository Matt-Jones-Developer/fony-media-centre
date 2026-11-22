//! Decides whether a TMDB search result is the same show as a folder. Plain functions, no network.
//! The rule is tapehead's: never guess. Anything short of one clear match stays unmatched.

use super::tmdb::SearchResult;

/// The TMDB id when exactly one result has the same name (or original name) and first air year.
/// No year on our side means no match: a name alone is too easy to get wrong.
pub fn pick_exact(name: &str, year: Option<i32>, results: &[SearchResult]) -> Option<i64> {
    let year = year?;
    let wanted = comparable(name);
    let mut matches = Vec::new();
    for result in results {
        let same_name = comparable(&result.name) == wanted
            || result
                .original_name
                .as_deref()
                .is_some_and(|original| comparable(original) == wanted);
        if same_name && result.first_air_year == Some(year) {
            matches.push(result.tmdb_id);
        }
    }
    let [only] = matches.as_slice() else {
        return None;
    };
    Some(*only)
}

/// Lower case, "&" read as "and", only letters and numbers kept: "Chip 'n' Dale & Co" becomes "chipndaleandco".
fn comparable(name: &str) -> String {
    name.to_lowercase()
        .replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(tmdb_id: i64, name: &str, first_air_year: Option<i32>) -> SearchResult {
        SearchResult {
            tmdb_id,
            name: name.into(),
            original_name: None,
            first_air_year,
        }
    }

    #[test]
    fn accepts_one_exact_name_and_year() {
        let results = [result(553, "Dungeons & Dragons", Some(1983))];
        assert_eq!(
            pick_exact("Dungeons & Dragons", Some(1983), &results),
            Some(553)
        );
    }

    #[test]
    fn ignores_case_punctuation_and_ampersands() {
        let results = [result(4229, "Dexter's Laboratory", Some(1996))];
        assert_eq!(
            pick_exact("dexters laboratory", Some(1996), &results),
            Some(4229)
        );
        let results = [result(553, "Dungeons and Dragons", Some(1983))];
        assert_eq!(
            pick_exact("Dungeons & Dragons", Some(1983), &results),
            Some(553)
        );
    }

    #[test]
    fn accepts_a_match_on_the_original_name() {
        let mut found = result(9, "Les Mondes Engloutis", Some(1985));
        found.original_name = Some("Spartakus and the Sun Beneath the Sea".into());
        assert_eq!(
            pick_exact(
                "Spartakus and the Sun Beneath the Sea",
                Some(1985),
                &[found]
            ),
            Some(9)
        );
    }

    #[test]
    fn refuses_two_exact_matches() {
        let results = [
            result(282_192, "Little Miss", Some(1983)),
            result(126_227, "Little Miss", Some(1983)),
        ];
        assert_eq!(pick_exact("Little Miss", Some(1983), &results), None);
    }

    #[test]
    fn refuses_a_different_year_a_different_name_or_no_year() {
        let results = [
            result(32_605, "The Looney Tunes Show", Some(2011)),
            result(1, "Looney Tunes", Some(2011)),
        ];
        assert_eq!(pick_exact("Looney Tunes", None, &results), None);
        assert_eq!(pick_exact("Looney Tunes", Some(1930), &results), None);
        assert_eq!(pick_exact("Looney", Some(2011), &results), None);
    }
}

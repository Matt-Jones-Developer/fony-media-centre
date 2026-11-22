//! App settings, kept in the database so Rust (and later the Pi build) reads the same choices as the UI.

use super::Library;
use rusqlite::{OptionalExtension, params};

/// How handwritten tape labels look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LabelStyle {
    /// Kalam: neat felt-tip handwriting.
    Regular,
    /// Sedgwick Ave Display: a chunky graffiti tag. The default.
    Graf,
}

impl LabelStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            LabelStyle::Regular => "regular",
            LabelStyle::Graf => "graf",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "regular" => Some(LabelStyle::Regular),
            "graf" => Some(LabelStyle::Graf),
            _ => None,
        }
    }
}

impl Library {
    /// The saved label style, or Graf when nothing (or something unknown) is saved.
    pub fn label_style(&self) -> Result<LabelStyle, String> {
        let saved: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'label_style'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| format!("Can't read settings: {err}"))?;
        Ok(saved
            .as_deref()
            .and_then(LabelStyle::parse)
            .unwrap_or(LabelStyle::Graf))
    }

    pub fn save_label_style(&mut self, style: LabelStyle) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES ('label_style', ?1)
                 ON CONFLICT (key) DO UPDATE SET
                   value = excluded.value, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                params![style.as_str()],
            )
            .map(|_| ())
            .map_err(|err| format!("Can't save settings: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_style_defaults_to_graf_and_remembers_a_change() {
        let mut library = Library::open_in_memory().unwrap();
        assert_eq!(library.label_style().unwrap(), LabelStyle::Graf);

        library.save_label_style(LabelStyle::Regular).unwrap();
        assert_eq!(library.label_style().unwrap(), LabelStyle::Regular);
        library.save_label_style(LabelStyle::Graf).unwrap();
        assert_eq!(library.label_style().unwrap(), LabelStyle::Graf);
    }

    #[test]
    fn only_known_label_styles_are_accepted() {
        assert_eq!(LabelStyle::parse("graf"), Some(LabelStyle::Graf));
        assert_eq!(LabelStyle::parse("regular"), Some(LabelStyle::Regular));
        assert_eq!(LabelStyle::parse("Comic Sans"), None);
        assert_eq!(LabelStyle::parse(""), None);
    }
}

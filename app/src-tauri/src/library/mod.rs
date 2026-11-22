//! The media library: what's on the drives, kept in SQLite. See docs/ARCHITECTURE.md "Library core".

pub mod db;
pub mod markers;
pub mod parse;
pub mod probe;
pub mod scan;

//! The library database. This is the only module that writes to it.
//! It lives in Application Support, never on a media drive.

mod metadata_store;
mod probe_store;
mod scan_store;

use crate::drives::VolumeInfo;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::fs;
use std::path::Path;

pub struct Library {
    conn: Connection,
}

pub use metadata_store::MetadataCounts;
pub use probe_store::ProbeJob;
pub use scan_store::LibraryCounts;

/// One drive as the UI sees it: is it plugged in, and which of its folders does the library read.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveSummary {
    pub id: i64,
    pub name: String,
    pub volume_uuid: String,
    pub mount_point: String,
    pub is_online: bool,
    pub folders: Vec<String>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .map_err(|err| format!("Can't create {}: {err}", folder.display()))?;
        }
        let conn =
            Connection::open(path).map_err(|err| format!("Can't open the library: {err}"))?;
        Self::setup(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        Self::setup(Connection::open_in_memory().map_err(|err| err.to_string())?)
    }

    fn setup(conn: Connection) -> Result<Self, String> {
        // WAL lets the UI read while a scan is writing. SQLite leaves foreign keys off unless asked.
        conn.pragma_update(None, "journal_mode", "WAL")
            .and_then(|_| conn.pragma_update(None, "foreign_keys", "ON"))
            .and_then(|_| conn.pragma_update(None, "busy_timeout", 5000))
            .map_err(|err| format!("Can't set up the library: {err}"))?;
        let mut library = Library { conn };
        library.migrate()?;
        Ok(library)
    }

    /// SQLite's `user_version` counts how many migrations have run. Each new one runs once, in order.
    fn migrate(&mut self) -> Result<(), String> {
        let migrations = [
            include_str!("migrations/001_library.sql"),
            include_str!("migrations/002_metadata.sql"),
        ];

        let done: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|err| format!("Can't read the library version: {err}"))?;

        for (number, sql) in (1_i64..).zip(migrations) {
            if number <= done {
                continue;
            }
            let tx = self.conn.transaction().map_err(|err| err.to_string())?;
            tx.execute_batch(sql)
                .and_then(|_| tx.pragma_update(None, "user_version", number))
                .and_then(|_| tx.commit())
                .map_err(|err| format!("Library migration {number} failed: {err}"))?;
        }
        Ok(())
    }

    /// Records the drive (by UUID) and one of its folders. Adding the same folder again changes nothing.
    pub fn add_library_folder(
        &mut self,
        volume: &VolumeInfo,
        rel_path: &str,
        kind: &str,
    ) -> Result<(), String> {
        let uuid = volume
            .volume_uuid
            .as_deref()
            .ok_or("This drive has no volume UUID, so the library can't recognise it later")?;
        let name = volume.volume_name.as_deref().unwrap_or("Unnamed drive");

        let tx = self.conn.transaction().map_err(|err| err.to_string())?;
        tx.execute(
            "INSERT INTO drives (volume_uuid, name, last_mount_point, last_seen_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             ON CONFLICT (volume_uuid) DO UPDATE SET
               name = excluded.name,
               last_mount_point = excluded.last_mount_point,
               last_seen_at = excluded.last_seen_at,
               updated_at = excluded.last_seen_at",
            params![uuid, name, volume.mount_point],
        )
        .map_err(|err| format!("Can't save the drive: {err}"))?;
        tx.execute(
            "INSERT INTO library_roots (drive_id, rel_path, kind)
             SELECT id, ?2, ?3 FROM drives WHERE volume_uuid = ?1
             ON CONFLICT (drive_id, rel_path) DO NOTHING",
            params![uuid, rel_path, kind],
        )
        .map_err(|err| format!("Can't save the folder: {err}"))?;
        tx.commit().map_err(|err| err.to_string())
    }

    /// Updates the name and mount point of every known drive that's plugged in right now.
    pub fn record_mounted(&mut self, mounted: &[VolumeInfo]) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|err| err.to_string())?;
        for volume in mounted {
            let Some(uuid) = volume.volume_uuid.as_deref() else {
                continue;
            };
            let name = volume.volume_name.as_deref().unwrap_or("Unnamed drive");
            tx.execute(
                "UPDATE drives SET
                   name = ?2,
                   last_mount_point = ?3,
                   last_seen_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE volume_uuid = ?1",
                params![uuid, name, volume.mount_point],
            )
            .map_err(|err| format!("Can't update the drive: {err}"))?;
        }
        tx.commit().map_err(|err| err.to_string())
    }

    /// Every known drive. A drive is online when its UUID is in `mounted`.
    pub fn drives(&self, mounted: &[VolumeInfo]) -> Result<Vec<DriveSummary>, String> {
        let mut drives = self.load_drives()?;
        for drive in &mut drives {
            drive.is_online = mounted
                .iter()
                .any(|volume| volume.volume_uuid.as_deref() == Some(drive.volume_uuid.as_str()));
        }

        let mut folders = self
            .conn
            .prepare("SELECT drive_id, rel_path FROM library_roots ORDER BY rel_path")
            .map_err(|err| err.to_string())?;
        let rows = folders
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;
        for row in rows {
            let (drive_id, rel_path) = row.map_err(|err| err.to_string())?;
            if let Some(drive) = drives.iter_mut().find(|drive| drive.id == drive_id) {
                drive.folders.push(rel_path);
            }
        }
        Ok(drives)
    }

    fn load_drives(&self) -> Result<Vec<DriveSummary>, String> {
        let mut query = self
            .conn
            .prepare("SELECT id, name, volume_uuid, last_mount_point FROM drives ORDER BY name")
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok(DriveSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    volume_uuid: row.get(2)?,
                    mount_point: row.get(3)?,
                    is_online: false,
                    folders: Vec::new(),
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<_, _>>()
            .map_err(|err| format!("Can't read the drives: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_drive() -> VolumeInfo {
        VolumeInfo {
            mount_point: "/Volumes/Glitchy_2TB".into(),
            volume_name: Some("Glitchy_2TB".into()),
            volume_uuid: Some("D2946B44-EEF5-3A81-A7BE-648CD3312449".into()),
        }
    }

    #[test]
    fn runs_every_migration_once() {
        let mut library = Library::open_in_memory().unwrap();
        library.migrate().unwrap();
        let version: i64 = library
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
    }

    #[test]
    fn adding_the_same_folder_twice_keeps_one_drive_and_one_folder() {
        let mut library = Library::open_in_memory().unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();

        let drives = library.drives(&[]).unwrap();
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].folders, vec!["MEDIA/Cartoons"]);
    }

    #[test]
    fn a_drive_is_online_only_while_its_uuid_is_mounted() {
        let mut library = Library::open_in_memory().unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();

        assert!(library.drives(&[media_drive()]).unwrap()[0].is_online);
        assert!(!library.drives(&[]).unwrap()[0].is_online);
    }

    #[test]
    fn a_renamed_drive_is_still_the_same_drive() {
        let mut library = Library::open_in_memory().unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();

        let renamed = VolumeInfo {
            mount_point: "/Volumes/Tapes".into(),
            volume_name: Some("Tapes".into()),
            ..media_drive()
        };
        library
            .record_mounted(std::slice::from_ref(&renamed))
            .unwrap();

        let drives = library.drives(&[renamed]).unwrap();
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].name, "Tapes");
        assert_eq!(drives[0].mount_point, "/Volumes/Tapes");
        assert!(drives[0].is_online);
    }

    #[test]
    fn a_drive_without_a_uuid_is_refused() {
        let mut library = Library::open_in_memory().unwrap();
        let no_uuid = VolumeInfo {
            volume_uuid: None,
            ..media_drive()
        };
        assert!(
            library
                .add_library_folder(&no_uuid, "MEDIA/Cartoons", "cartoons")
                .is_err()
        );
    }
}

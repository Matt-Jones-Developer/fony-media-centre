//! Which files still need ffprobe, and saving what it found.

use super::Library;
use crate::drives::VolumeInfo;
use crate::library::probe::ProbeInfo;
use rusqlite::params;
use std::path::PathBuf;

pub struct ProbeJob {
    pub file_id: i64,
    /// Where the file is on disk right now.
    pub path: PathBuf,
}

impl Library {
    /// Unprobed files on drives that are plugged in, in path order.
    pub fn files_to_probe(&self, mounted: &[VolumeInfo]) -> Result<Vec<ProbeJob>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT files.id, files.rel_path, drives.volume_uuid
                 FROM files JOIN drives ON drives.id = files.drive_id
                 WHERE files.probed_at IS NULL
                 ORDER BY files.rel_path",
            )
            .map_err(|err| err.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut jobs = Vec::new();
        for row in rows {
            let (file_id, rel_path, uuid) = row.map_err(|err| err.to_string())?;
            let Some(volume) = mounted
                .iter()
                .find(|volume| volume.volume_uuid.as_deref() == Some(uuid.as_str()))
            else {
                continue;
            };
            jobs.push(ProbeJob {
                file_id,
                path: PathBuf::from(&volume.mount_point).join(rel_path),
            });
        }
        Ok(jobs)
    }

    pub fn save_probes(&mut self, results: &[(i64, ProbeInfo)]) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|err| err.to_string())?;
        for (file_id, info) in results {
            tx.prepare_cached(
                "UPDATE files SET
                   duration_ms = ?2, video_codec = ?3, width = ?4, height = ?5,
                   probed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?1",
            )
            .and_then(|mut update| {
                update.execute(params![
                    file_id,
                    info.duration_ms,
                    info.video_codec,
                    info.width,
                    info.height
                ])
            })
            .map_err(|err| format!("Can't save ffprobe results: {err}"))?;
        }
        tx.commit()
            .map_err(|err| format!("Can't save ffprobe results: {err}"))
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

    fn library_with_one_file() -> Library {
        let mut library = Library::open_in_memory().unwrap();
        library
            .add_library_folder(&media_drive(), "MEDIA/Cartoons", "cartoons")
            .unwrap();
        library
            .conn
            .execute(
                "INSERT INTO files (drive_id, rel_path, size, modified_at)
                 VALUES (1, 'MEDIA/Cartoons/Show/Season 01/E1.mkv', 100, '2026-11-22T00:00:00Z')",
                [],
            )
            .unwrap();
        library
    }

    #[test]
    fn lists_unprobed_files_only_on_plugged_in_drives() {
        let library = library_with_one_file();
        let jobs = library.files_to_probe(&[media_drive()]).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].path,
            PathBuf::from("/Volumes/Glitchy_2TB/MEDIA/Cartoons/Show/Season 01/E1.mkv")
        );
        assert!(library.files_to_probe(&[]).unwrap().is_empty());
    }

    #[test]
    fn a_saved_probe_takes_the_file_off_the_list() {
        let mut library = library_with_one_file();
        let job = library.files_to_probe(&[media_drive()]).unwrap().remove(0);
        let info = ProbeInfo {
            duration_ms: Some(1_294_760),
            video_codec: Some("h264".into()),
            width: Some(640),
            height: Some(480),
        };
        library.save_probes(&[(job.file_id, info)]).unwrap();

        assert!(library.files_to_probe(&[media_drive()]).unwrap().is_empty());
        let duration: i64 = library
            .conn
            .query_row("SELECT duration_ms FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(duration, 1_294_760);
    }
}

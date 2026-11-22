//! Finds which drive a path lives on and reads that drive's volume UUID.
//! The UUID stays the same if the drive is renamed or mounted somewhere else.

use serde::Serialize;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub mount_point: String,
    pub volume_name: Option<String>,
    pub volume_uuid: Option<String>,
}

/// Every drive mounted under /Volumes that has a UUID. Ones that can't be read are skipped.
pub fn mounted_volumes() -> Vec<VolumeInfo> {
    let Ok(entries) = fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    let mut volumes = Vec::new();
    for entry in entries.flatten() {
        let Ok(volume) = volume_info(&entry.path()) else {
            continue;
        };
        if volume.volume_uuid.is_some() {
            volumes.push(volume);
        }
    }
    volumes
}

/// Where `folder` sits inside the drive mounted at `mount_point`, e.g. "MEDIA/Cartoons".
pub fn path_on_drive(folder: &Path, mount_point: &Path) -> Result<String, String> {
    let relative = folder.strip_prefix(mount_point).map_err(|_| {
        format!(
            "{} isn't on the drive at {}",
            folder.display(),
            mount_point.display()
        )
    })?;
    let relative = relative
        .to_str()
        .ok_or("The folder name isn't valid text")?;
    if relative.is_empty() {
        return Err("Pick a folder inside the drive, not the whole drive".into());
    }
    Ok(relative.to_string())
}

pub fn volume_info(path: &Path) -> Result<VolumeInfo, String> {
    let path =
        fs::canonicalize(path).map_err(|err| format!("Can't read {}: {err}", path.display()))?;
    let mount = mount_point(&path)?;

    let output = Command::new("/usr/sbin/diskutil")
        .args(["info", "-plist"])
        .arg(&mount)
        .output()
        .map_err(|err| format!("Couldn't run diskutil: {err}"))?;
    let plist = String::from_utf8_lossy(&output.stdout);

    Ok(VolumeInfo {
        mount_point: mount.display().to_string(),
        volume_name: plist_string(&plist, "VolumeName"),
        volume_uuid: plist_string(&plist, "VolumeUUID"),
    })
}

/// Every file on one drive shares a device number. Walk up the folders until the
/// device number changes: the last folder before that is where the drive is mounted.
fn mount_point(path: &Path) -> Result<PathBuf, String> {
    let device = device_of(path)?;
    let mut mount = path.to_path_buf();
    for parent in path.ancestors().skip(1) {
        if device_of(parent)? != device {
            break;
        }
        mount = parent.to_path_buf();
    }
    Ok(mount)
}

fn device_of(path: &Path) -> Result<u64, String> {
    fs::metadata(path)
        .map(|metadata| metadata.dev())
        .map_err(|err| format!("Can't read {}: {err}", path.display()))
}

/// diskutil's plist has pairs like `<key>VolumeUUID</key><string>ABC-123</string>`.
fn plist_string(plist: &str, key: &str) -> Option<String> {
    let after_key = plist.split(&format!("<key>{key}</key>")).nth(1)?;
    let value = after_key.trim_start().strip_prefix("<string>")?;
    Some(value.split("</string>").next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_string_value_from_a_plist() {
        let plist = "<dict>\n\t<key>VolumeName</key>\n\t<string>Glitchy_2TB</string>\n</dict>";
        assert_eq!(
            plist_string(plist, "VolumeName").as_deref(),
            Some("Glitchy_2TB")
        );
        assert_eq!(plist_string(plist, "VolumeUUID"), None);
    }

    #[test]
    fn gives_a_folder_path_relative_to_its_drive() {
        let mount = Path::new("/Volumes/Glitchy_2TB");
        assert_eq!(
            path_on_drive(Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons"), mount).unwrap(),
            "MEDIA/Cartoons"
        );
        assert!(path_on_drive(mount, mount).is_err());
        assert!(path_on_drive(Path::new("/Users/glitchy81"), mount).is_err());
    }

    #[test]
    fn finds_the_root_as_the_mount_point_of_the_system_drive() {
        let mount = mount_point(Path::new("/usr/bin")).unwrap();
        assert!(mount == Path::new("/") || mount.starts_with("/System/Volumes"));
    }

    /// Needs the media drive plugged in: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_the_media_drive_uuid_from_a_folder_inside_it() {
        let info = volume_info(Path::new("/Volumes/Glitchy_2TB/MEDIA/Cartoons")).unwrap();
        assert_eq!(info.mount_point, "/Volumes/Glitchy_2TB");
        let uuid = info.volume_uuid.expect("a volume UUID");
        assert_eq!(uuid.len(), 36);
        println!("{} = {uuid}", info.volume_name.unwrap_or_default());
    }
}

//! Windows-specific search directories for J-Link.

use std::path::PathBuf;

pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Ok(profile) = std::env::var("USERPROFILE") {
        dirs.push(PathBuf::from(&profile).join("AppData").join("Roaming").join("SEGGER"));
    }
    dirs.push(PathBuf::from(r"C:\Program Files\SEGGER"));
    dirs.push(PathBuf::from(r"C:\Program Files (x86)\SEGGER"));
    dirs
}
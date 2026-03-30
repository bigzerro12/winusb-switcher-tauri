//! Linux-specific search directories for J-Link.

use std::path::PathBuf;

pub fn search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/SEGGER/JLink"),
    ]
}
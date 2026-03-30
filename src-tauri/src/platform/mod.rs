//! Platform abstraction for PATH management and JLink search directories.

use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod linux;

pub struct PlatformConfig {
    pub jlink_bin: &'static str,
    pub jlink_executable: &'static str,
}

pub fn config() -> PlatformConfig {
    #[cfg(target_os = "windows")]
    return PlatformConfig {
        jlink_bin: "JLink",
        jlink_executable: "JLink.exe",
    };
    #[cfg(target_os = "macos")]
    return PlatformConfig {
        jlink_bin: "JLinkExe",
        jlink_executable: "JLinkExe",
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return PlatformConfig {
        jlink_bin: "JLinkExe",
        jlink_executable: "JLinkExe",
    };
}

pub fn search_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    return windows::search_dirs();
    #[cfg(target_os = "macos")]
    return macos::search_dirs();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return linux::search_dirs();
}

/// Find directory containing JLink executable in known locations.
pub fn find_jlink_in_search_dirs() -> Option<PathBuf> {
    let executable = config().jlink_executable;
    for base in search_dirs() {
        if !base.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(executable).exists() {
                    return Some(path);
                }
            }
        }
        if base.join(executable).exists() {
            return Some(base);
        }
    }
    None
}

/// Update current process PATH so the new dir is usable in this session.
pub fn prepend_to_process_path(dir: &str) {
    let path_key = std::env::vars()
        .find(|(k, _)| k.to_lowercase() == "path")
        .map(|(k, _)| k)
        .unwrap_or_else(|| "PATH".to_string());

    let current = std::env::var(&path_key).unwrap_or_default();
    if !current.to_lowercase().contains(&dir.to_lowercase()) {
        let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
        std::env::set_var(&path_key, format!("{}{}{}", current, separator, dir));
    }
}

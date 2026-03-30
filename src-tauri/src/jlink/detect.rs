//! J-Link installation detection.

use std::path::Path;
use crate::jlink::{runner, scripts, types::InstallStatus};
use crate::platform;

pub fn detect() -> InstallStatus {
    log::info!("[jlink] Detecting installation...");
    let cfg = platform::config();

    if let Ok((stdout, _)) = runner::run(cfg.jlink_bin, scripts::detect()) {
        if let Some(version) = runner::parse_version(&stdout) {
            log::info!("[jlink] Found in PATH: {}", version);
            return InstallStatus {
                installed: true,
                path: which::which(cfg.jlink_bin)
                    .ok()
                    .map(|p| p.to_string_lossy().to_string()),
                version: Some(version),
            };
        }
    }

    log::info!("[jlink] Not found in PATH, scanning search dirs...");

    if let Some(dir) = platform::find_jlink_in_search_dirs() {
        return detect_from_dir(&dir, cfg.jlink_bin, cfg.jlink_executable);
    }

    log::info!("[jlink] J-Link not found");
    InstallStatus { installed: false, path: None, version: None }
}

fn detect_from_dir(dir: &Path, global_bin: &str, executable: &str) -> InstallStatus {
    let full_path = dir.join(executable);
    let full_path_str = full_path.to_string_lossy().to_string();
    let dir_str = dir.to_string_lossy().to_string();

    log::info!("[jlink] Found at: {}", full_path_str);
    // Add the directory to this process's in-memory PATH so all subsequent
    // JLink invocations can use the short name ("JLink") without a full path.
    // No changes are made to the user or system environment.
    platform::prepend_to_process_path(&dir_str);

    if let Ok((stdout, _)) = runner::run(global_bin, scripts::detect()) {
        if let Some(version) = runner::parse_version(&stdout) {
            log::info!("[jlink] Running via process PATH: {}", global_bin);
            return InstallStatus { installed: true, path: Some(full_path_str), version: Some(version) };
        }
    }

    log::info!("[jlink] Global command not available — using full path");
    let version = runner::run(&full_path_str, scripts::detect())
        .ok()
        .and_then(|(stdout, _)| runner::parse_version(&stdout));

    InstallStatus { installed: true, path: Some(full_path_str), version }
}
//! Types for download and install operations.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub percent: u32,
    pub transferred: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub cancelled: Option<bool>,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanInstallerResult {
    pub found: bool,
    pub path: String,
    pub message: String,
}

/// Platform-specific download configuration.
pub struct DownloadConfig {
    /// Direct download URL for the installer
    pub url: &'static str,
    /// Save path for download (.tmp extension during download)
    pub save_tmp: PathBuf,
    /// Final path after rename (.exe/.pkg/.deb)
    pub save_final: PathBuf,
    /// Scan path where app looks for existing installer (Windows `scan_for_installer` only)
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub scan_path: PathBuf,
}

impl DownloadConfig {
    pub fn for_platform() -> Self {
        if cfg!(target_os = "windows") {
            let segger = std::env::var("USERPROFILE")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default())
                .join("AppData").join("Roaming").join("SEGGER");
            let (url, filename): (&'static str, &'static str) = if cfg!(target_arch = "aarch64") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_Windows_arm64.exe",
                    "JLink_Windows_arm64.exe",
                )
            } else if cfg!(target_arch = "x86") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_Windows.exe",
                    "JLink_Windows.exe",
                )
            } else {
                // x86_64 — default
                (
                    "https://www.segger.com/downloads/jlink/JLink_Windows_x86_64.exe",
                    "JLink_Windows_x86_64.exe",
                )
            };
            let save_final = segger.join(filename);
            DownloadConfig {
                url,
                save_tmp:  save_final.with_extension("tmp"),
                scan_path: save_final.clone(),
                save_final,
            }
        } else if cfg!(target_os = "macos") {
            let dl = dirs::download_dir().unwrap_or_default();
            let (url, filename): (&'static str, &'static str) = if cfg!(target_arch = "aarch64") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_MacOSX_arm64.pkg",
                    "JLink_MacOSX_arm64.pkg",
                )
            } else if cfg!(target_arch = "x86_64") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_MacOSX.pkg",
                    "JLink_MacOSX.pkg",
                )
            } else {
                // Universal fallback
                (
                    "https://www.segger.com/downloads/jlink/JLink_MacOSX_universal.pkg",
                    "JLink_MacOSX_universal.pkg",
                )
            };
            let save_final = dl.join(filename);
            DownloadConfig {
                url,
                save_tmp:  save_final.with_extension("tmp"),
                scan_path: save_final.clone(),
                save_final,
            }
        } else {
            // Linux
            let dl = dirs::download_dir().unwrap_or_default();
            let (url, filename): (&'static str, &'static str) = if cfg!(target_arch = "aarch64") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_Linux_arm64.deb",
                    "JLink_Linux_arm64.deb",
                )
            } else if cfg!(target_arch = "arm") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_Linux_arm.deb",
                    "JLink_Linux_arm.deb",
                )
            } else if cfg!(target_arch = "x86") {
                (
                    "https://www.segger.com/downloads/jlink/JLink_Linux_i386.deb",
                    "JLink_Linux_i386.deb",
                )
            } else {
                // x86_64 — default
                (
                    "https://www.segger.com/downloads/jlink/JLink_Linux_x86_64.deb",
                    "JLink_Linux_x86_64.deb",
                )
            };
            let save_final = dl.join(filename);
            DownloadConfig {
                url,
                save_tmp:  save_final.with_extension("tmp"),
                scan_path: save_final.clone(),
                save_final,
            }
        }
    }
}

/// Platform-specific cached installer path in app-local data.
///
/// Linux/macOS use this location as the source of truth after download
/// so cancellation logic can safely clean up Downloads without losing cache.
pub fn cached_installer_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local").join("share"));
    let filename = if cfg!(target_os = "macos") {
        "JLink_installer.pkg"
    } else if cfg!(target_os = "windows") {
        "JLink_installer.exe"
    } else {
        "JLink_installer.deb"
    };
    data_dir.join("com.probeconfigurator.app").join(filename)
}
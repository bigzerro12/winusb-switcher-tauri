//! Platform-specific J-Link installation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::download::types::InstallResult;
use crate::platform;
use crate::process::NoWindow;

pub async fn install(
    installer_path: &str,
    cancelled: &'static AtomicBool,
) -> Result<InstallResult, String> {
    if cfg!(target_os = "windows") {
        install_windows(installer_path, cancelled).await
    } else if cfg!(target_os = "macos") {
        install_macos(installer_path).await
    } else {
        install_linux(installer_path).await
    }
}

// ─── Windows ──────────────────────────────────────────────────────────────────

async fn install_windows(
    installer_path: &str,
    cancelled: &'static AtomicBool,
) -> Result<InstallResult, String> {
    let dest = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default())
        .join("AppData").join("Roaming").join("SEGGER");

    let installer = installer_path.to_string();
    let dest_str = dest.to_string_lossy().to_string();

    // Spawn PowerShell with -Wait so we can cancel by killing the child
    let ps_cmd = format!(
        "Start-Process -FilePath '{}' -ArgumentList '/S','/D={}' -Verb RunAs -Wait",
        installer.replace('\'', "`'"),
        dest_str.replace('\'', "`'")
    );

    let mut child = match std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .no_window()
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Ok(InstallResult {
            success: false, cancelled: Some(true),
            message: format!("Failed to launch installer: {}", e), path: None,
        }),
    };

    let install_start_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let start = std::time::Instant::now();
    // True once the PowerShell -Wait process has exited (installer fully done).
    let mut ps_exited = false;
    let mut dll_check_ticker: u64 = 0;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Handle cancel
        if cancelled.load(Ordering::SeqCst) {
            let pid = child.id();
            let _ = child.kill();
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .no_window()
                .status();
            log::info!("[install] Cancelled (pid={})", pid);
            return Ok(InstallResult {
                success: false, cancelled: Some(true),
                message: "Installation cancelled.".to_string(), path: None,
            });
        }

        // Check if PowerShell exited
        if !ps_exited {
            match child.try_wait() {
                Ok(Some(s)) if !s.success() => {
                    return Ok(InstallResult {
                        success: false, cancelled: Some(true),
                        message: "UAC denied or installation failed.".to_string(), path: None,
                    });
                }
                Ok(Some(_)) => {
                    ps_exited = true; // PowerShell -Wait exited → installer fully done
                    log::info!("[install] PowerShell exited — checking for J-Link");
                }
                Ok(None) => {}    // Still running
                Err(e) => return Ok(InstallResult {
                    success: false, cancelled: None,
                    message: format!("Process error: {}", e), path: None,
                }),
            }
        }

        // Check DLL every ~1s while the installer is running.
        // Once PS has exited, check every cycle (installer is done, just waiting for FS).
        dll_check_ticker += 1;
        if dll_check_ticker % 3 == 0 || ps_exited {
            if let Some(dir) = platform::find_jlink_in_search_dirs() {
                let dll = dir.join("JLink_x64.dll");
                let dll_exists = std::fs::metadata(&dll).is_ok();

                // While the installer is still running, require a fresh DLL mtime to avoid
                // falsely accepting a pre-existing install before the new one is complete.
                // Once PowerShell (-Wait) has returned, the installer is done — the DLL
                // merely existing is sufficient (reinstalling the same version won't update mtimes).
                let accept = if ps_exited {
                    dll_exists
                } else {
                    std::fs::metadata(&dll)
                        .and_then(|m| m.modified())
                        .map(|mtime| {
                            mtime.duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() >= install_start_ms
                        })
                        .unwrap_or(false)
                };

                if accept {
                    let dir_str = dir.to_string_lossy().to_string();
                    log::info!("[install] Complete: {}", dir_str);
                    return Ok(InstallResult {
                        success: true, cancelled: None,
                        message: "J-Link installed successfully.".to_string(),
                        path: Some(dir_str),
                    });
                }

                // PS exited but DLL not yet visible — give a brief grace period then fail.
                if ps_exited && start.elapsed().as_secs() > 15 {
                    log::warn!("[install] PowerShell exited but J-Link DLL not found after grace period");
                    return Ok(InstallResult {
                        success: false, cancelled: None,
                        message: "Installation finished but J-Link was not detected. Please try again.".to_string(),
                        path: None,
                    });
                }
            } else if ps_exited && start.elapsed().as_secs() > 15 {
                // PS exited but no J-Link directory found at all.
                log::warn!("[install] PowerShell exited but no J-Link directory found");
                return Ok(InstallResult {
                    success: false, cancelled: None,
                    message: "Installation finished but J-Link was not detected. Please try again.".to_string(),
                    path: None,
                });
            }
        }

        if start.elapsed().as_secs() > 120 {
            let _ = child.kill();
            return Ok(InstallResult {
                success: false, cancelled: None,
                message: "Installation timed out.".to_string(), path: None,
            });
        }
    }
}

// ─── macOS ────────────────────────────────────────────────────────────────────

async fn install_macos(installer_path: &str) -> Result<InstallResult, String> {
    let cmd = format!(
        "do shell script \"installer -pkg \\\"{}\\\" -target /\" with administrator privileges",
        installer_path
    );
    match std::process::Command::new("osascript").args(["-e", &cmd]).status() {
        Ok(s) if s.success() => Ok(InstallResult {
            success: true, cancelled: None,
            message: "J-Link installed successfully.".to_string(),
            path: platform::find_jlink_in_search_dirs()
                .map(|d| d.to_string_lossy().to_string()),
        }),
        Ok(_) => Ok(InstallResult {
            success: false, cancelled: Some(true),
            message: "Installation cancelled.".to_string(), path: None,
        }),
        Err(e) => Ok(InstallResult {
            success: false, cancelled: None,
            message: format!("Install failed: {}", e), path: None,
        }),
    }
}

// ─── Linux ───────────────────────────────────────────────────────────────────

async fn install_linux(installer_path: &str) -> Result<InstallResult, String> {
    match std::process::Command::new("pkexec")
        .args(["dpkg", "-i", installer_path])
        .status()
    {
        Ok(s) if s.success() => Ok(InstallResult {
            success: true, cancelled: None,
            message: "J-Link installed successfully.".to_string(),
            path: platform::find_jlink_in_search_dirs()
                .map(|d| d.to_string_lossy().to_string()),
        }),
        Ok(s) if s.code() == Some(126) => Ok(InstallResult {
            success: false, cancelled: Some(true),
            message: "Installation cancelled.".to_string(), path: None,
        }),
        _ => Ok(InstallResult {
            success: false, cancelled: None,
            message: "Installation failed.".to_string(), path: None,
        }),
    }
}
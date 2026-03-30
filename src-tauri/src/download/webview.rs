//! Hidden WebviewWindow download for SEGGER J-Link.
//!
//! Platform notes:
//! - Windows: DownloadEvent::Requested fires → destination override works → poll .tmp file
//! - Linux/macOS: DownloadEvent::Requested does NOT fire reliably on WebKitGTK
//!                → use Finished event path + time-based progress animation

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl};
use tauri::webview::DownloadEvent;
use crate::download::types::DownloadProgress;
use crate::error::{AppError, AppResult};

/// Track the actual file being downloaded so cancel can delete it precisely
static CURRENT_DOWNLOAD_PATH: Mutex<Option<String>> = Mutex::new(None);
/// Track last successful download path (from any generation) as fallback
static LAST_SUCCESS_PATH: Mutex<Option<String>> = Mutex::new(None);
/// Track when download started (unix timestamp seconds)
static DOWNLOAD_START_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Generation counter — incremented each download to ignore stale Finished events
static DOWNLOAD_GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn clear_current_download_path() {
    if let Ok(mut p) = CURRENT_DOWNLOAD_PATH.lock() {
        *p = None;
    }
}

pub fn get_download_start_secs() -> u64 {
    DOWNLOAD_START_SECS.load(Ordering::SeqCst)
}

pub fn start_download(
    app: &AppHandle,
    save_tmp: PathBuf,
    save_final: PathBuf,
    download_url: &'static str,
    cancelled: &'static AtomicBool,
    download_complete: &'static AtomicBool,
) -> AppResult<()> {
    let app_clone = app.clone();
    #[cfg(target_os = "windows")]
    let save_tmp_clone = save_tmp.clone();
    #[cfg(target_os = "windows")]
    let _ = &save_final;
    #[cfg(not(target_os = "windows"))]
    let save_final_clone = save_final.clone();

    // Reset tracked download path and record start time
    clear_current_download_path();
    if let Ok(mut p) = LAST_SUCCESS_PATH.lock() { *p = None; }
    let start_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    DOWNLOAD_START_SECS.store(start_secs, Ordering::SeqCst);
    let my_gen = DOWNLOAD_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    // Close ALL previous jlink-downloader windows
    for label in (1..my_gen).map(|g| format!("jlink-downloader-{}", g)) {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.close();
        }
    }

    // Linux/macOS: start progress animation immediately
    // Windows: progress comes from poll task
    #[cfg(not(target_os = "windows"))]
    {
        let app_anim = app.clone();
        let anim_gen = my_gen;
        std::thread::spawn(move || {
            const EXPECTED_MS: f64 = 35_000.0;
            let start = std::time::Instant::now();

            loop {
                std::thread::sleep(std::time::Duration::from_millis(200));
                // Stop if a newer download has started
                if DOWNLOAD_GENERATION.load(Ordering::SeqCst) != anim_gen { break; }
                if download_complete.load(Ordering::SeqCst) { break; }
                if cancelled.load(Ordering::SeqCst) { break; }

                let elapsed = start.elapsed().as_millis() as f64;
                let t = (elapsed / EXPECTED_MS).min(1.0);
                let eased = 1.0 - (1.0 - t).powi(3);
                let percent = ((eased * 95.0) as u32).max(1);

                app_anim.emit("download://progress", DownloadProgress {
                    percent, transferred: 0, total: 0,
                }).ok();
            }
        });
    }

    let my_gen_capture = my_gen;
    let window_label = format!("jlink-downloader-{}", my_gen);
    tauri::WebviewWindowBuilder::new(
        app,
        &window_label,
        WebviewUrl::External(download_url.parse().expect("valid SEGGER URL")),
    )
    .title("Downloading J-Link...")
    .visible(false)
    .inner_size(800.0, 600.0)
    .initialization_script(r#"
        if (!window.__seggerAccepted) {
            window.__seggerAccepted = true;
            window.addEventListener('load', () => {
                setTimeout(() => {
                    document.querySelectorAll('input[type="checkbox"]').forEach(cb => {
                        if (!cb.checked) {
                            cb.checked = true;
                            cb.dispatchEvent(new Event('change', { bubbles: true }));
                        }
                    });
                    const btn = document.querySelector(
                        'input[type="submit"], button[type="submit"], button[type="button"]'
                    );
                    if (btn) btn.click();
                }, 2000);
            });
        }
    "#)
    .on_download(move |_webview, event| {
        let ext = if cfg!(target_os = "windows") { ".exe" }
                  else if cfg!(target_os = "macos") { ".pkg" }
                  else { ".deb" };

        match event {
            DownloadEvent::Requested { url, destination } => {
                let url_str = url.to_string();
                if !url_str.contains("JLink") || !url_str.ends_with(ext) {
                    return false;
                }
                log::info!("[download] Requested: {}", url_str);

                // Windows: override destination → .tmp file, poll task handles rest
                #[cfg(target_os = "windows")]
                {
                    *destination = save_tmp_clone.clone();
                    if let Ok(mut p) = CURRENT_DOWNLOAD_PATH.lock() {
                        *p = Some(save_tmp_clone.to_string_lossy().to_string());
                    }
                    app_clone.emit("download://progress", DownloadProgress {
                        percent: 0, transferred: 0, total: 64_602_096,
                    }).ok();
                    return true;
                }

                // Linux/macOS: let WebView download normally (animation already running)
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = destination;
                    true
                }
            }

            DownloadEvent::Finished { url: _, path, success } => {
                let current_gen = DOWNLOAD_GENERATION.load(Ordering::SeqCst);
                log::info!("[download] Finished: success={}, gen={}/{}, path={:?}", success, my_gen_capture, current_gen, path);

                // Store successful path from any generation as fallback
                if success {
                    if let Some(ref p) = path {
                        if let Ok(mut lsp) = LAST_SUCCESS_PATH.lock() {
                            *lsp = Some(p.to_string_lossy().to_string());
                        }
                    }
                }

                // Ignore stale events from previous download attempts
                if my_gen_capture != current_gen {
                    log::warn!("[download] Ignoring stale Finished event (gen {} != {})", my_gen_capture, current_gen);
                    return false;
                }

                if success {
                    #[cfg(target_os = "windows")]
                    {
                        // On Windows the poll task owns the completion event.
                        // Just note that WebView finished so the poll task can proceed.
                        // (compare_exchange so we don't clobber if HTTP already won)
                        let _ = download_complete.compare_exchange(
                            false, true, Ordering::SeqCst, Ordering::SeqCst,
                        );
                        close_downloader_window(&app_clone, my_gen_capture);
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        // Use compare_exchange: if HTTP fast path already completed,
                        // discard this WebView result silently to avoid a double-complete.
                        let won = download_complete.compare_exchange(
                            false, true, Ordering::SeqCst, Ordering::SeqCst,
                        ).is_ok();
                        if !won {
                            log::info!("[download] WebView Finished — HTTP fast path already won, discarding");
                            close_downloader_window(&app_clone, my_gen_capture);
                            return false;
                        }

                        // Use the file WebView already downloaded
                        let final_path = if let Some(ref actual) = path {
                            actual.to_string_lossy().to_string()
                        } else {
                            save_final_clone.to_string_lossy().to_string()
                        };

                        let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
                        log::info!("[download] Complete: {} ({} bytes)", final_path, size);

                        // If cancelled while WebKitGTK was buffering → delete the file now
                        if cancelled.load(Ordering::SeqCst) {
                            let _ = std::fs::remove_file(&final_path);
                            log::info!("[download] Deleted post-buffer (was cancelled): {}", final_path);
                            app_clone.emit("download://cancelled", ()).ok();
                            return false;
                        }

                        // Copy file to safe location (app data dir) to prevent
                        // cancel_download from deleting it after Finished fires
                        let safe_path = if let Some(data_dir) = dirs::data_local_dir() {
                            let cached = crate::download::types::cached_installer_path();
                            let safe = if cached.starts_with(&data_dir) {
                                cached
                            } else {
                                let fallback_name = save_final_clone
                                    .file_name()
                                    .map(|n| n.to_owned())
                                    .unwrap_or_else(|| std::ffi::OsString::from("JLink_installer.bin"));
                                data_dir.join("com.probeconfigurator.app").join(fallback_name)
                            };
                            if let Some(parent) = safe.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if std::fs::copy(&final_path, &safe).is_ok() {
                                log::info!("[download] Copied to safe location: {}", safe.display());
                                // Remove original from Downloads — safe location is the source of truth
                                let _ = std::fs::remove_file(&final_path);
                                log::info!("[download] Removed from Downloads: {}", final_path);
                                safe.to_string_lossy().to_string()
                            } else {
                                final_path.clone()
                            }
                        } else {
                            final_path.clone()
                        };

                        // Store path so cancel_download can delete it if needed
                        if let Ok(mut p) = CURRENT_DOWNLOAD_PATH.lock() {
                            *p = Some(safe_path.clone());
                        }

                        app_clone.emit("download://progress", DownloadProgress {
                            percent: 100, transferred: size, total: size,
                        }).ok();
                        app_clone.emit("download://completed", &safe_path).ok();

                        close_downloader_window(&app_clone, my_gen_capture);
                    }
                } else {
                    log::error!("[download] Failed (cancelled={})", cancelled.load(Ordering::SeqCst));

                    if !cancelled.load(Ordering::SeqCst) {
                        // Try fallback: use last successful download from a stale generation
                        let fallback = LAST_SUCCESS_PATH.lock().ok()
                            .and_then(|p| p.clone())
                            .filter(|p| {
                                std::path::Path::new(p).exists() &&
                                std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) > 10_000_000
                            });

                        if let Some(fallback_path) = fallback {
                            let size = std::fs::metadata(&fallback_path).map(|m| m.len()).unwrap_or(0);
                            log::info!("[download] Using fallback path: {} ({} bytes)", fallback_path, size);
                            download_complete.store(true, Ordering::SeqCst);
                            app_clone.emit("download://progress", DownloadProgress {
                                percent: 100, transferred: size, total: size,
                            }).ok();
                            app_clone.emit("download://completed", &fallback_path).ok();
                            return false;
                        }
                    }

                    download_complete.store(false, Ordering::SeqCst);

                    if cancelled.load(Ordering::SeqCst) {
                        app_clone.emit("download://cancelled", ()).ok();
                    } else {
                        // WebKitGTK session corrupted — auto retry with fresh window
                        log::warn!("[download] Session failed, auto-retrying with fresh WebView...");

                        // Temporarily set cancelled=true to stop old animation threads
                        cancelled.store(true, Ordering::SeqCst);

                        app_clone.emit("download://retrying", ()).ok();

                        let app_retry = app_clone.clone();
                        std::thread::spawn(move || {
                            // Wait for animation threads to see cancelled=true and stop
                            std::thread::sleep(std::time::Duration::from_millis(400));

                            // Close all existing downloader windows
                            for g in 1..=20u32 {
                                let lbl = format!("jlink-downloader-{}", g);
                                if let Some(w) = app_retry.get_webview_window(&lbl) {
                                    let _ = w.close();
                                }
                            }
                            // Signal frontend to retry (will reset cancelled flag via download_jlink)
                            app_retry.emit("download://retry", ()).ok();
                        });
                    }
                }
                false
            }
            _ => true,
        }
    })
    .build()
    .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    Ok(())
}

fn close_downloader_window(app: &AppHandle, gen: u32) {
    let label = format!("jlink-downloader-{}", gen);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.close();
    }
}
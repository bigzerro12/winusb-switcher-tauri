//! File size polling task — detects when a WebView2 download completes.
//!
//! WebView2's `DownloadEvent::Finished` is unreliable in dev mode (Vite),
//! so we poll the destination file until its size stabilizes.
//!
//! The `.tmp` → `.exe` rename trick prevents WebView2 from deleting the
//! file during cleanup (WebView2 only tracks the original `.tmp` path).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use crate::download::types::DownloadProgress;

/// Spawn a background polling task that monitors download progress.
///
/// - Emits `download://progress` events as file grows
/// - Renames `.tmp` → `.exe` when stable
/// - Emits `download://completed` with final path
/// - Exits cleanly on cancel or generation mismatch (stale task)
#[allow(dead_code)]
pub fn spawn(
    app: AppHandle,
    save_tmp: PathBuf,
    save_final: PathBuf,
    cancelled: &'static AtomicBool,
    generation: &'static AtomicU32,
    my_generation: u32,
    expected_size: u64,
) {
    let save_final_str = save_final.to_string_lossy().to_string();

    tokio::task::spawn(async move {
        let mut last_size: u64 = 0;
        let mut stable_count: u8 = 0;
        let start = std::time::Instant::now();

        // Brief wait for download to start
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

            // Exit if superseded by a newer download attempt
            if generation.load(Ordering::SeqCst) != my_generation {
                log::info!("[poll] Stale generation — exiting");
                return;
            }

            if cancelled.load(Ordering::SeqCst) {
                log::info!("[poll] Cancelled — exiting");
                return;
            }

            if start.elapsed().as_secs() > 300 {
                log::warn!("[poll] Timeout after 300s — exiting");
                return;
            }

            let size = std::fs::metadata(&save_tmp).map(|m| m.len()).unwrap_or(0);

            // Emit progress
            if size > 0 {
                let total = expected_size.max(size);
                let percent = ((size * 100 / total).min(99) as u32).max(1);
                app.emit("download://progress", DownloadProgress {
                    percent,
                    transferred: size,
                    total,
                }).ok();
            } else {
                app.emit("download://progress", DownloadProgress {
                    percent: 0,
                    transferred: 0,
                    total: expected_size,
                }).ok();
            }

            // Detect completion: size > 10MB and stable for 3 checks
            if size > 10_000_000 {
                if size == last_size {
                    stable_count += 1;
                    if stable_count >= 3 {
                        log::info!("[poll] Download complete: {} bytes", size);

                        // Rename .tmp → .exe (prevents WebView2 cleanup)
                        let final_path = match std::fs::rename(&save_tmp, &save_final) {
                            Ok(_) => {
                                log::info!("[poll] Renamed .tmp → {}", save_final.display());
                                save_final_str.clone()
                            }
                            Err(e) => {
                                log::warn!("[poll] Rename failed: {} — using .tmp path", e);
                                save_tmp.to_string_lossy().to_string()
                            }
                        };

                        // Close the hidden downloader window for this generation
                        let label = format!("jlink-downloader-{}", my_generation);
                        if let Some(w) = app.get_webview_window(&label) {
                            let _ = w.close();
                        }

                        app.emit("download://progress", DownloadProgress {
                            percent: 100,
                            transferred: size,
                            total: size,
                        }).ok();
                        app.emit("download://completed", &final_path).ok();
                        return;
                    }
                } else {
                    stable_count = 0;
                    last_size = size;
                }
            } else {
                last_size = size;
            }
        }
    });
}
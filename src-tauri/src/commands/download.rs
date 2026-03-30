//! Tauri commands for downloading and installing J-Link software.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use crate::download::{self, types::*};
use crate::error::AppError;

// ─── Global flags ─────────────────────────────────────────────────────────────

static DOWNLOAD_CANCELLED: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_COMPLETE: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_GENERATION: AtomicU32 = AtomicU32::new(0);
static INSTALL_CANCELLED: AtomicBool = AtomicBool::new(false);

// ─── Download commands ────────────────────────────────────────────────────────

/// Check if a cached installer exists.
/// Spec: only check app cache — Downloads folder is not user-visible.
#[tauri::command]
pub async fn scan_for_installer() -> Result<ScanInstallerResult, AppError> {
    #[cfg(target_os = "windows")]
    {
        let cfg = DownloadConfig::for_platform();
        let found = cfg.scan_path.exists();
        return Ok(ScanInstallerResult {
            found,
            path: if found { cfg.scan_path.to_string_lossy().to_string() } else { String::new() },
            message: if found { "Installer found".to_string() } else { "Not found".to_string() },
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let cache_path = download::types::cached_installer_path();

        let found = cache_path.exists()
            && std::fs::metadata(&cache_path).map(|m| m.len()).unwrap_or(0) > 10_000_000;

        Ok(ScanInstallerResult {
            found,
            path: if found { cache_path.to_string_lossy().to_string() } else { String::new() },
            message: if found { "Installer cached".to_string() } else { "Not found".to_string() },
        })
    }
}

/// Start downloading J-Link.
///
/// Strategy: WebView and direct HTTP start in parallel.
/// - WebView opens immediately (hidden, auto-accepts license).
/// - HTTP streams the binary directly; if SEGGER returns HTML (license gate) or fails,
///   the task exits silently — WebView continues uninterrupted.
/// - Whichever path completes first emits `download://completed`; the other exits cleanly.
///
/// Returns immediately — progress/completion signaled via events.
#[tauri::command]
pub async fn download_jlink(app: AppHandle) -> Result<serde_json::Value, AppError> {
    DOWNLOAD_CANCELLED.store(false, Ordering::SeqCst);
    DOWNLOAD_COMPLETE.store(false, Ordering::SeqCst);
    let gen = DOWNLOAD_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    // Short drain delay: lets stale Finished events from a previous cancelled download settle.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let cfg = DownloadConfig::for_platform();

    // Ensure save directory exists
    if let Some(parent) = cfg.save_tmp.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }

    // Remove leftovers from any previous attempt
    let _ = std::fs::remove_file(&cfg.save_tmp);
    let http_tmp = cfg.save_tmp.with_extension("http.tmp");
    let _ = std::fs::remove_file(&http_tmp);

    let save_str = cfg.save_final.to_string_lossy().to_string();

    // ── Path 1: WebView (always starts immediately) ────────────────────────────
    // Windows: saves to save_tmp (.tmp), poll task renames to save_final
    // Linux/macOS: WebKitGTK download → Finished event provides path
    download::webview::start_download(
        &app,
        cfg.save_tmp.clone(),
        cfg.save_final.clone(),
        cfg.url,
        &DOWNLOAD_CANCELLED,
        &DOWNLOAD_COMPLETE,
    )?;

    // ── Path 2: Direct HTTP (parallel fast path) ───────────────────────────────
    // Uses a separate .http.tmp file so it never collides with the WebView .tmp.
    // If HTTP wins: sets DOWNLOAD_COMPLETE via compare_exchange and emits completed.
    // If HTTP fails (SEGGER license gate, network error, etc.): logs at INFO and exits.
    // WebView continues regardless.
    {
        let app_http = app.clone();
        let http_tmp_path = http_tmp.clone();
        let final_path = cfg.save_final.clone();
        let url = cfg.url;
        tokio::spawn(async move {
            match download::http::download_to_path(
                &app_http, url, &http_tmp_path, &final_path, &DOWNLOAD_CANCELLED,
            ).await {
                Ok(()) => {
                    // HTTP won — mark complete and notify frontend (only if WebView hasn't already).
                    if DOWNLOAD_COMPLETE.compare_exchange(
                        false, true, Ordering::SeqCst, Ordering::SeqCst,
                    ).is_ok() {
                        log::info!("[download] Direct HTTP fast path completed first");
                        let _ = app_http.emit(
                            "download://completed",
                            final_path.to_string_lossy().to_string(),
                        );
                    }
                    // http_tmp was renamed to final on success; nothing to clean up.
                }
                Err(e) => {
                    // Expected for SEGGER (license gate). WebView is already running.
                    log::info!("[download] HTTP fast path unavailable ({}) — WebView will handle it", e);
                    let _ = tokio::fs::remove_file(&http_tmp_path).await;
                }
            }
        });
    }

    // ── Windows poll task ──────────────────────────────────────────────────────
    // Watches WebView's .tmp file; exits early if HTTP already marked DOWNLOAD_COMPLETE.
    #[cfg(target_os = "windows")]
    download::poll::spawn(
        app,
        cfg.save_tmp,
        cfg.save_final,
        &DOWNLOAD_CANCELLED,
        &DOWNLOAD_COMPLETE,
        &DOWNLOAD_GENERATION,
        gen,
        64_602_096,
    );
    #[cfg(not(target_os = "windows"))]
    let _ = gen;

    Ok(serde_json::json!({
        "success": true,
        "path": save_str,
        "mode": "http-or-webview"
    }))
}

/// Cancel an in-progress download.
#[tauri::command]
pub async fn cancel_download(app: AppHandle) -> Result<(), AppError> {
    log::info!("[download] cancel_download called");

    // Don't cancel if download already completed successfully
    if DOWNLOAD_COMPLETE.load(Ordering::SeqCst) {
        log::info!("[download] Download already complete — ignoring cancel");
        return Ok(());
    }

    DOWNLOAD_CANCELLED.store(true, Ordering::SeqCst);

    // Close all jlink-downloader windows (gen-labelled)
    let current_gen = DOWNLOAD_GENERATION.load(Ordering::SeqCst);
    for g in 1..=current_gen {
        let label = format!("jlink-downloader-{}", g);
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.close();
        }
    }

    let cfg = DownloadConfig::for_platform();

    // Delete WebView .tmp, HTTP .http.tmp, and any partially-written final file
    let http_tmp = cfg.save_tmp.with_extension("http.tmp");
    for p in [&cfg.save_tmp, &http_tmp, &cfg.save_final] {
        if p.exists() {
            let _ = std::fs::remove_file(p);
            log::info!("[download] Deleted on cancel: {}", p.display());
        }
    }

    // Linux/macOS: delete only JLink file created AFTER this download started
    #[cfg(not(target_os = "windows"))]
    {
        let ext = if cfg!(target_os = "macos") { ".pkg" } else { ".deb" };
        let download_dir = dirs::download_dir().unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_default().join("Downloads")
        });
        let start_secs = crate::download::webview::get_download_start_secs();

        let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
        if let Ok(entries) = std::fs::read_dir(&download_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("JLink") && (
                    name.ends_with(ext) ||
                    name.ends_with(&format!("{}.wkdownload", ext))
                ) {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(mtime) = meta.modified() {
                            let mtime_secs = mtime
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            if mtime_secs >= start_secs {
                                if name.ends_with(".wkdownload") {
                                    let _ = std::fs::remove_file(&path);
                                    log::info!("[download] Deleted partial on cancel: {:?}", path);
                                } else if newest.as_ref().map_or(true, |(_, t)| mtime > *t) {
                                    newest = Some((path, mtime));
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some((path, _)) = newest {
            let _ = std::fs::remove_file(&path);
            log::info!("[download] Deleted on cancel: {:?}", path);
        } else {
            log::info!("[download] No new JLink file found to delete (cancelled before download completed)");
        }
    }

    app.emit("download://cancelled", ()).ok();
    Ok(())
}

// ─── Install commands ─────────────────────────────────────────────────────────

/// Install J-Link from the given installer path.
#[tauri::command]
pub async fn install_jlink(installer_path: String) -> Result<InstallResult, AppError> {
    INSTALL_CANCELLED.store(false, Ordering::SeqCst);
    log::info!("[install] Installing: {}", installer_path);

    let file_size = std::fs::metadata(&installer_path).map(|m| m.len()).unwrap_or(0);
    if file_size < 10_000_000 {
        return Ok(InstallResult {
            success: false, cancelled: None,
            message: format!("File too small ({} bytes). Please re-download.", file_size),
            path: None,
        });
    }

    download::installer::install(&installer_path, &INSTALL_CANCELLED).await
        .map_err(|e| AppError::InstallFailed(e.to_string()))
}

/// Cancel an in-progress installation.
#[tauri::command]
pub async fn cancel_install(_keep_installer: bool) -> Result<(), AppError> {
    if INSTALL_CANCELLED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        log::info!("[install] cancel_install called — installer file kept");
    }
    Ok(())
}

// ─── Unused stubs (kept for API compatibility) ────────────────────────────────

#[tauri::command]
pub async fn open_download_webview(_app: AppHandle) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({ "success": false, "message": "Not used" }))
}

#[tauri::command]
pub async fn receive_download_chunk(
    _app: AppHandle,
    _chunk_b64: String,
    _transferred: u64,
    _total: u64,
    _done: bool,
) -> Result<(), AppError> {
    Ok(())
}

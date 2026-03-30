//! Direct HTTP download implementation (reqwest streaming).
//!
//! Used as a fast path before falling back to the WebView-based SEGGER flow.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::download::types::DownloadProgress;
use crate::error::{AppError, AppResult};

fn looks_like_binary(content_type: Option<&reqwest::header::HeaderValue>) -> bool {
    let ct = content_type.and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    if ct.is_empty() {
        // Some servers omit; treat as possibly binary.
        return true;
    }
    !(ct.contains("text/") || ct.contains("application/xhtml") || ct.contains("application/html"))
}

pub async fn download_to_path(
    app: &AppHandle,
    url: &str,
    out_tmp: &Path,
    out_final: &Path,
    cancelled: &'static AtomicBool,
) -> AppResult<()> {
    let client = reqwest::Client::builder()
        .user_agent("winusb-switcher/1.0 (tauri; reqwest)")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(AppError::DownloadFailed(format!(
            "HTTP {}",
            resp.status()
        )));
    }

    // If SEGGER responds with an HTML page (license gate), bail out so caller can fall back to WebView.
    if !looks_like_binary(resp.headers().get(reqwest::header::CONTENT_TYPE)) {
        return Err(AppError::DownloadFailed(
            "Server returned HTML/text (likely license gate)".to_string(),
        ));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut transferred: u64 = 0;

    if let Some(parent) = out_tmp.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;
    }
    let _ = tokio::fs::remove_file(out_tmp).await;
    let _ = tokio::fs::remove_file(out_final).await;

    let mut file = tokio::fs::File::create(out_tmp)
        .await
        .map_err(|e| AppError::Io(e.to_string()))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::SeqCst) {
            let _ = tokio::fs::remove_file(out_tmp).await;
            return Err(AppError::Cancelled);
        }
        let chunk = chunk.map_err(|e| AppError::DownloadFailed(e.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;
        transferred = transferred.saturating_add(chunk.len() as u64);

        let percent = if total > 0 {
            ((transferred.saturating_mul(100) / total).min(99) as u32).max(1)
        } else {
            // Unknown total; keep UI "alive"
            1
        };

        let _ = app.emit(
            "download://progress",
            DownloadProgress {
                percent,
                transferred,
                total,
            },
        );
    }

    file.flush()
        .await
        .map_err(|e| AppError::Io(e.to_string()))?;

    // Atomically move into place where possible.
    tokio::fs::rename(out_tmp, out_final)
        .await
        .map_err(|e| AppError::Io(e.to_string()))?;

    let size = tokio::fs::metadata(out_final)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let _ = app.emit(
        "download://progress",
        DownloadProgress {
            percent: 100,
            transferred: size,
            total: size.max(total),
        },
    );

    Ok(())
}


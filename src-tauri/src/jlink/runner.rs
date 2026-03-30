//! Low-level JLink CLI execution helper.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::error::{AppError, AppResult};
use crate::process::NoWindow;

/// If JLink does not exit within this many seconds it is forcibly killed.
/// Normal operations finish in <2 s; this guards against a probe that stops
/// responding mid-session (which would otherwise hang detect_and_scan forever).
const RUNNER_TIMEOUT_SECS: u64 = 15;

/// Execute JLink with given stdin input, return (stdout, stderr).
pub fn run(bin: &str, input: &str) -> AppResult<(String, String)> {
    log::debug!("[jlink] Running: {} -NoGUI 1", bin);
    log::debug!("[jlink] Input:\n{}", input);

    let mut child = Command::new(bin)
        .args(["-NoGUI", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_window()
        .spawn()
        .map_err(|e| AppError::JLinkNotFound(e.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())
            .map_err(|e| AppError::JLinkFailed(e.to_string()))?;
    }

    // Watchdog: kill the JLink process if it doesn't exit within RUNNER_TIMEOUT_SECS.
    // wait_with_output() has no built-in timeout — a stuck probe (e.g. during
    // selectprobe / firmware fetch) would otherwise block detect_and_scan forever,
    // leaving the frontend permanently on "Checking J-Link installation...".
    let pid = child.id();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_watcher = timed_out.clone();
    std::thread::spawn(move || {
        if done_rx.recv_timeout(std::time::Duration::from_secs(RUNNER_TIMEOUT_SECS)).is_err() {
            // Timeout or sender dropped unexpectedly — kill the process.
            timed_out_watcher.store(true, Ordering::Relaxed);
            log::warn!("[jlink] Watchdog: JLink pid={} still running after {}s — killing", pid, RUNNER_TIMEOUT_SECS);
            #[cfg(target_os = "windows")]
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .no_window()
                .status();
            #[cfg(not(target_os = "windows"))]
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    });

    let output = child.wait_with_output()
        .map_err(|e| AppError::JLinkFailed(e.to_string()))?;

    // Notify the watchdog that the process already exited so it skips the kill.
    let _ = done_tx.send(());

    if timed_out.load(Ordering::Relaxed) {
        log::warn!("[jlink] Runner: returned partial output after timeout ({} bytes stdout)", output.stdout.len());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::debug!("[jlink] stdout: {}", &stdout[..stdout.len().min(500)]);
    if !stderr.is_empty() {
        log::debug!("[jlink] stderr: {}", &stderr[..stderr.len().min(200)]);
    }

    Ok((stdout, stderr))
}

/// Parse SEGGER J-Link Commander version from banner output.
pub fn parse_version(output: &str) -> Option<String> {
    for line in output.lines() {
        if line.contains("SEGGER J-Link Commander") {
            if let Some(v) = line.split('V').nth(1) {
                let ver = v.split_whitespace().next().unwrap_or("").to_string();
                if !ver.is_empty() {
                    return Some(format!("SEGGER J-Link Commander V{}", ver));
                }
            }
        }
    }
    None
}
//! Global application state managed by Tauri's state system.

use std::sync::Mutex;

/// Resolved JLink binary path, cached after first successful detection.
/// Default is the platform's global command name (e.g. "JLink" on Windows).
pub struct JLinkState {
    pub bin: Mutex<String>,
}

impl JLinkState {
    pub fn new(default_bin: &str) -> Self {
        Self {
            bin: Mutex::new(default_bin.to_string()),
        }
    }

    pub fn get(&self) -> String {
        self.bin.lock().unwrap().clone()
    }

    pub fn set(&self, bin: String) {
        *self.bin.lock().unwrap() = bin;
    }
}
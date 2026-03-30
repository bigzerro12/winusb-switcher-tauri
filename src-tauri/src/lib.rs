//! WinUSB Switcher — Tauri application entry point.

mod commands;
mod download;
mod error;
mod jlink;
mod platform;
mod process;
mod state;

use commands::{download as dl, probe};
use state::JLinkState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_bin = platform::config().jlink_bin;

    tauri::Builder::default()
        .manage(JLinkState::new(default_bin))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("winusb_switcher_lib", log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Probe
            probe::detect_and_scan,
            probe::scan_probes,
            probe::switch_usb_driver,
            probe::get_arch_info,
            // Download / Install
            dl::scan_for_installer,
            dl::download_jlink,
            dl::cancel_download,
            dl::install_jlink,
            dl::cancel_install,
            // Stubs
            dl::open_download_webview,
            dl::receive_download_chunk,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
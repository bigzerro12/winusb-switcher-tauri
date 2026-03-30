//! JLink CLI command script builders.
//!
//! All JLink stdin command sequences are defined here.
//! This makes it easy to track, modify, and test command flows in one place.
//!
//! JLink Commander accepts commands via stdin when launched with `-NoGUI 1`.
//! Commands are newline-separated. `exit` terminates the session.

// ─── Detection ────────────────────────────────────────────────────────────────

/// Probe version banner — used to detect JLink and parse version string.
pub fn detect() -> &'static str {
    "\n\nExit\n"
}

// ─── Scan ─────────────────────────────────────────────────────────────────────

/// List all connected probes.
pub fn show_emu_list() -> &'static str {
    "ShowEmuList\nExit\n"
}

/// Fetch firmware dates for N probes by selecting each one by index.
pub fn fetch_firmware_dates(count: usize) -> String {
    let mut s = String::from("exec DisableAutoUpdateFW\n");
    for i in 0..count {
        s.push_str(&format!("selectprobe\n{}\n", i));
    }
    s.push_str("exit\n");
    s
}

// ─── USB Driver ───────────────────────────────────────────────────────────────

/// Session 1: switch probe to WinUSB driver via WebUSBEnable.
///
/// Must be followed by set_usb_driver_reboot in a separate session.
/// Expected output: "Probe configured successfully."
pub fn set_usb_driver_webusb(probe_index: usize) -> String {
    format!(
        "exec DisableAutoUpdateFW\nselectprobe\n{}\nWebUSBEnable\nsleep 100\nexit\n",
        probe_index
    )
}

/// Session 1: switch probe back to SEGGER USB driver via WebUSBDisable.
///
/// Must be followed by set_usb_driver_reboot in a separate session.
/// Expected output: "Probe configured successfully."
pub fn set_usb_driver_segger(probe_index: usize) -> String {
    format!(
        "exec DisableAutoUpdateFW\nselectprobe\n{}\nWebUSBDisable\nsleep 100\nexit\n",
        probe_index
    )
}

/// Session 1 fallback: switch probe to WinUSB driver via WinUSBEnable.
///
/// Used when WebUSBEnable is not supported by the installed J-Link version.
pub fn set_usb_driver_winusb_enable(probe_index: usize) -> String {
    format!(
        "exec DisableAutoUpdateFW\nselectprobe\n{}\nWinUSBEnable\nsleep 100\nexit\n",
        probe_index
    )
}

/// Session 1 fallback: switch probe back to SEGGER USB driver via WinUSBDisable.
///
/// Used when WebUSBDisable is not supported by the installed J-Link version.
pub fn set_usb_driver_winusb_disable(probe_index: usize) -> String {
    format!(
        "exec DisableAutoUpdateFW\nselectprobe\n{}\nWinUSBDisable\nsleep 100\nexit\n",
        probe_index
    )
}

/// Session 2: reboot probe so USB driver change takes effect.
///
/// Must be a fresh J-Link session after the WebUSB / WinUSB enable-disable step.
pub fn set_usb_driver_reboot(probe_index: usize) -> String {
    format!(
        "exec DisableAutoUpdateFW\nselectprobe\n{}\nreboot\nsleep 100\nexit\n",
        probe_index
    )
}
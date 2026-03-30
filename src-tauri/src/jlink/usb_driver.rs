//! USB driver switching operations (WinUSB / SEGGER).

use crate::jlink::{runner, scripts, types::{UsbDriverMode, UsbDriverResult}};

pub fn switch(bin: &str, probe_index: usize, mode: UsbDriverMode) -> UsbDriverResult {
    log::info!("[jlink] Switching probe[{}] USB driver to {:?}", probe_index, mode);

    // ── Session 1: try WebUSBEnable / WebUSBDisable first ───────────────────────
    let primary_input = match mode {
        UsbDriverMode::WinUsb => scripts::set_usb_driver_webusb(probe_index),
        UsbDriverMode::Segger => scripts::set_usb_driver_segger(probe_index),
    };

    let primary_stdout = match runner::run(bin, &primary_input) {
        Ok((stdout, _)) => stdout,
        Err(e) => {
            return UsbDriverResult {
                success: false,
                error: Some(e.to_string()),
                reboot_not_supported: false,
            };
        }
    };

    log::debug!("[jlink] usb_driver primary write stdout:\n{}", primary_stdout);

    let mut write_stdout = primary_stdout;
    if command_not_supported(&write_stdout) {
        log::warn!(
            "[jlink] WebUSB command unsupported for probe[{}], retrying with WinUSB* command",
            probe_index
        );

        let fallback_input = match mode {
            UsbDriverMode::WinUsb => scripts::set_usb_driver_winusb_enable(probe_index),
            UsbDriverMode::Segger => scripts::set_usb_driver_winusb_disable(probe_index),
        };

        write_stdout = match runner::run(bin, &fallback_input) {
            Ok((stdout, _)) => stdout,
            Err(e) => {
                return UsbDriverResult {
                    success: false,
                    error: Some(e.to_string()),
                    reboot_not_supported: false,
                };
            }
        };

        log::debug!("[jlink] usb_driver fallback write stdout:\n{}", write_stdout);
    }

    if !write_succeeded(&write_stdout) {
        if command_not_supported(&write_stdout) {
            return UsbDriverResult {
                success: false,
                error: Some(
                    "This J-Link version does not support USB driver switching commands."
                        .to_string(),
                ),
                reboot_not_supported: false,
            };
        }
        return UsbDriverResult {
            success: false,
            error: Some(format!(
                "Unexpected response from J-Link: {}",
                write_stdout.lines().last().unwrap_or("(no output)")
            )),
            reboot_not_supported: false,
        };
    }

    log::info!("[jlink] Probe[{}] USB driver configured, starting reboot session", probe_index);

    // ── Session 2: reboot probe ────────────────────────────────────────────────
    let reboot_input = scripts::set_usb_driver_reboot(probe_index);
    let mut reboot_not_supported = false;
    match runner::run(bin, &reboot_input) {
        Ok((stdout, _)) => {
            log::debug!("[jlink] usb_driver reboot stdout:\n{}", stdout);
            if stdout.contains("Command not supported by connected probe.") {
                reboot_not_supported = true;
                log::warn!(
                    "[jlink] Probe[{}] reboot command not supported (USB driver switch)",
                    probe_index
                );
            } else if stdout.contains("Rebooted successfully") {
                log::info!("[jlink] Probe[{}] rebooted, USB driver switch complete", probe_index);
            } else {
                log::warn!("[jlink] Probe[{}] reboot not confirmed after USB driver switch", probe_index);
            }
        }
        Err(e) => {
            log::warn!("[jlink] Probe[{}] reboot session failed after USB driver switch: {}", probe_index, e);
        }
    }

    UsbDriverResult {
        success: true,
        error: None,
        reboot_not_supported,
    }
}

fn command_not_supported(stdout: &str) -> bool {
    stdout.contains("Unknown command")
        || stdout.contains("Syntax error")
        || stdout.contains("not supported")
}

fn write_succeeded(stdout: &str) -> bool {
    stdout.contains("Probe configured successfully")
}
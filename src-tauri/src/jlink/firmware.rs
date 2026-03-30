//! Firmware update operations.

use crate::jlink::{runner, scripts, types::FirmwareUpdateResult};

pub fn update(bin: &str, probe_index: usize) -> FirmwareUpdateResult {
    let input = scripts::update_firmware(probe_index);
    log::info!("[jlink] Updating firmware for probe[{}]...", probe_index);

    match runner::run(bin, &input) {
        Ok((stdout, _)) => {
            let firmware = stdout
                .lines()
                .find(|l| l.contains("Firmware:") && l.contains("compiled"))
                .and_then(|l| l.find("compiled ").map(|p| l[p + 9..].trim().to_string()))
                .unwrap_or_default();

            if firmware.is_empty() {
                return FirmwareUpdateResult::Failed {
                    error: "Could not parse firmware version from output".to_string(),
                };
            }

            if stdout.contains("New firmware booted successfully") {
                log::info!("[jlink] Probe[{}] updated → {}", probe_index, firmware);
                FirmwareUpdateResult::Updated { firmware }
            } else {
                log::info!("[jlink] Probe[{}] already current: {}", probe_index, firmware);
                FirmwareUpdateResult::Current { firmware }
            }
        }
        Err(e) => {
            log::error!("[jlink] updateFirmware failed for probe[{}]: {}", probe_index, e);
            FirmwareUpdateResult::Failed { error: e.to_string() }
        }
    }
}


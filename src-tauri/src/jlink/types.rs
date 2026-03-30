//! Shared types for JLink operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub id: String,
    #[serde(rename = "serialNumber")]
    pub serial_number: String,
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "nickName")]
    pub nick_name: String,
    pub provider: String,
    pub connection: String,
    pub driver: String,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDriverResult {
    pub success: bool,
    pub error: Option<String>,
    /// True when reboot command returned "Command not supported by connected probe."
    #[serde(default)]
    pub reboot_not_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum UsbDriverMode {
    WinUsb,
    Segger,
}
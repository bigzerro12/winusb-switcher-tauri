// ─── Probe Types ──────────────────────────────────────────────────────────────

export type DriverType = "SEGGER" | "WinUSB" | "Unknown";
export type ProviderType = "JLink";

export type Probe = {
  id: string;
  serialNumber: string;
  productName: string;
  nickName: string;
  provider: ProviderType;
  connection: string;
  driver: DriverType;
  firmware?: string;
};

export type InstallStatus = {
  installed: boolean;
  path?: string;
  version?: string;
};

// ─── Error Types ──────────────────────────────────────────────────────────────
// Matches AppError enum in Rust (error.rs)

export type AppErrorKind =
  | "jLinkNotFound"
  | "jLinkFailed"
  | "downloadFailed"
  | "installFailed"
  | "cancelled"
  | "platform"
  | "io"
  | "internal";

export type AppError = {
  kind: AppErrorKind;
  message: string;
};

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e;
}

// ─── Result Types ─────────────────────────────────────────────────────────────

export type InstallResult = {
  success: boolean;
  cancelled?: boolean;
  message: string;
  path?: string;
};

export type ScanInstallerResult = {
  found: boolean;
  path: string;
  message: string;
};

export type DownloadProgress = {
  percent: number;
  transferred: number;
  total: number;
};

export type UsbDriverMode = "winUsb" | "segger";

export type UsbDriverResult = {
  success: boolean;
  error?: string;
  /** When true, reboot is not available on probe firmware — omit "may reboot briefly" in UI */
  rebootNotSupported?: boolean;
};

/** Returned by the `get_arch_info` command. Values match `std::env::consts` on the Rust side. */
export type ArchInfo = {
  /** e.g. "windows" | "macos" | "linux" */
  os: string;
  /** e.g. "x86_64" | "x86" | "aarch64" | "arm" */
  arch: string;
};

// ─── Tauri Command Names ──────────────────────────────────────────────────────

export const COMMANDS = {
  // Probe
  DETECT_AND_SCAN:    "detect_and_scan",
  SCAN_PROBES:        "scan_probes",
  SWITCH_USB_DRIVER:  "switch_usb_driver",
  GET_ARCH_INFO:      "get_arch_info",
  // Download / Install
  SCAN_FOR_INSTALLER: "scan_for_installer",
  DOWNLOAD_JLINK:     "download_jlink",
  CANCEL_DOWNLOAD:    "cancel_download",
  INSTALL_JLINK:      "install_jlink",
  CANCEL_INSTALL:     "cancel_install",
} as const;

// ─── Tauri Event Names ────────────────────────────────────────────────────────

export const EVENTS = {
  DOWNLOAD_PROGRESS:       "download://progress",
  DOWNLOAD_COMPLETED:      "download://completed",
  DOWNLOAD_CANCELLED:      "download://cancelled",
  DOWNLOAD_SESSION_FAILED: "download://session_failed",
  DOWNLOAD_RETRY:          "download://retry",
  DOWNLOAD_RETRYING:       "download://retrying",
} as const;
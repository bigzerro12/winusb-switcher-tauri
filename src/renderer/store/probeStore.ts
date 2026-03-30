import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import {
  Probe,
  InstallStatus,
  UsbDriverMode,
  UsbDriverResult,
  DriverType,
  COMMANDS,
} from "@shared/types";

function applyDriverOverrides(
  probes: Probe[],
  overrides: Record<string, DriverType>
): Probe[] {
  return probes.map((probe) => ({
    ...probe,
    driver: overrides[probe.id] ?? probe.driver,
  }));
}

function preserveSelection(
  probes: Probe[],
  selectedProbeId: string | null
): string | null {
  if (!selectedProbeId) return null;
  return probes.some((probe) => probe.id === selectedProbeId) ? selectedProbeId : null;
}

function resetUsbOperationStatus() {
  return {
    usbDriverStatus: "idle" as const,
    usbDriverMessage: "",
  };
}

interface ProbeState {
  probes: Probe[];
  driverOverrides: Record<string, DriverType>;
  isLoading: boolean;
  isInstalled: boolean | null;
  installPath: string | undefined;
  installVersion: string;
  selectedProbeId: string | null;
  error: string | null;
  usbDriverStatus: "idle" | "switching" | "success" | "failed";
  usbDriverMessage: string;

  checkInstallation: () => Promise<void>;
  scanProbesSilent: () => Promise<void>;
  scanProbes: () => Promise<void>;
  selectProbe: (id: string | null) => void;
  switchUsbDriver: (probeIndex: number, mode: UsbDriverMode) => Promise<void>;
}

export const useProbeStore = create<ProbeState>((set, get) => ({
  probes: [],
  driverOverrides: {},
  isLoading: false,
  isInstalled: null,
  installPath: undefined,
  installVersion: "",
  selectedProbeId: null,
  error: null,
  usbDriverStatus: "idle",
  usbDriverMessage: "",

  checkInstallation: async () => {
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<{
        status: InstallStatus;
        probes: Probe[];
      }>(COMMANDS.DETECT_AND_SCAN);

      const overrides = get().driverOverrides;
      const probes = applyDriverOverrides(result.probes, overrides);
      const selectedProbeId = preserveSelection(probes, get().selectedProbeId);

      set({
        isInstalled: result.status.installed,
        installPath: result.status.path,
        installVersion: result.status.version ?? "",
        probes,
        isLoading: false,
        selectedProbeId,
        ...resetUsbOperationStatus(),
      });
    } catch (err) {
      set({
        isInstalled: false,
        error: err instanceof Error ? err.message : String(err),
        isLoading: false,
      });
    }
  },

  scanProbes: async () => {
    set({
      isLoading: true,
      error: null,
      ...resetUsbOperationStatus(),
    });
    try {
      const overrides = get().driverOverrides;
      const probes = applyDriverOverrides(await invoke<Probe[]>(COMMANDS.SCAN_PROBES), overrides);
      const selectedProbeId = preserveSelection(probes, get().selectedProbeId);
      set({ probes, selectedProbeId, isLoading: false });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        isLoading: false,
      });
    }
  },

  scanProbesSilent: async () => {
    try {
      const overrides = get().driverOverrides;
      const probes = applyDriverOverrides(await invoke<Probe[]>(COMMANDS.SCAN_PROBES), overrides);
      const selectedProbeId = preserveSelection(probes, get().selectedProbeId);
      set({ probes, selectedProbeId });
    } catch { /* ignore */ }
  },

  selectProbe: (id) => {
    const current = get().selectedProbeId;
    set({
      selectedProbeId: current === id ? null : id,
      ...resetUsbOperationStatus(),
    });
  },

  switchUsbDriver: async (probeIndex, mode) => {
    set({
      usbDriverStatus: "switching",
      usbDriverMessage: mode === "winUsb"
        ? "Updating probe firmware and switching the USB driver to WinUSB..."
        : "Updating probe firmware and switching the USB driver to SEGGER...",
      error: null,
    });

    try {
      const result = await invoke<UsbDriverResult>(COMMANDS.SWITCH_USB_DRIVER, { probeIndex, mode });

      if (!result.success) {
        set({
          usbDriverStatus: "failed",
          usbDriverMessage: result.error ?? "Could not switch the USB driver.",
        });
        return;
      }

      const probes = get().probes;
      const probe = probes[probeIndex];
      if (probe) {
        const newDriver: DriverType = mode === "winUsb" ? "WinUSB" : "SEGGER";
        set({
          driverOverrides: { ...get().driverOverrides, [probe.id]: newDriver },
          probes: probes.map((p, i) => (i === probeIndex ? { ...p, driver: newDriver } : p)),
        });
      }

      await new Promise((resolve) => setTimeout(resolve, 900));
      await get().scanProbesSilent();

      const unplug = " You may need to unplug and replug your probe to apply the configuration changes.";
      const withRebootBrief = " The probe may reboot briefly.";
      const winMsg =
        "Switched to WinUSB." +
        (result.rebootNotSupported ? "" : withRebootBrief) +
        unplug;
      const seggerMsg =
        "Switched to SEGGER." +
        (result.rebootNotSupported ? "" : withRebootBrief) +
        unplug;
      set({
        usbDriverStatus: "success",
        usbDriverMessage: mode === "winUsb" ? winMsg : seggerMsg,
      });
    } catch (err) {
      set({
        usbDriverStatus: "failed",
        usbDriverMessage: err instanceof Error ? err.message : String(err),
      });
    }
  },
}));

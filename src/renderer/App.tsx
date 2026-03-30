import { useEffect } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useProbeStore } from "./store/probeStore";
import Dashboard from "./pages/Dashboard";
import InstallJLink from "./pages/InstallJLink";

const BODY_PADDING = 24; // 12px top + 12px bottom (matches body padding in styles.css)

// Resize only the window HEIGHT to fit content; preserves whatever width the user has set.
async function resizeHeightToContent() {
  await new Promise<void>((r) => setTimeout(r, 80)); // wait one paint for DOM to settle
  const card =
    document.querySelector<HTMLElement>(".app-card") ??
    document.querySelector<HTMLElement>(".message-card");
  if (!card) return;
  const targetHeight = card.scrollHeight + BODY_PADDING;
  try {
    const win = getCurrentWindow();
    // outerSize() is in physical pixels; divide by scaleFactor to get logical pixels
    const [physicalSize, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
    const currentLogicalWidth = Math.round(physicalSize.width / scale);
    await win.setSize(new LogicalSize(currentLogicalWidth, targetHeight));
  } catch (e) {
    console.error("[App] window resize failed:", e);
  }
}

export default function App() {
  const { isInstalled, isLoading, checkInstallation } = useProbeStore();

  useEffect(() => {
    // MUST call checkInstallation on mount
    // MUST catch errors — uncaught Promise rejections = blank screen
    checkInstallation().catch((err) => {
      console.error("[App] checkInstallation failed:", err);
    });
  }, []); // empty deps — run once on mount only

  // Auto-resize the window to match card content whenever the page changes
  useEffect(() => {
    if (isInstalled === null) return; // still loading — skip
    resizeHeightToContent();
  }, [isInstalled]);

  // Phase 1: still checking — show spinner
  if (isInstalled === null) {
    return (
      <div className="flex items-center justify-center h-screen bg-white">
        <div className="text-gray-400 text-sm">Checking J-Link installation...</div>
      </div>
    );
  }

  // Phase 2: checked but not installed
  if (!isInstalled) {
    return <InstallJLink />;
  }

  // Phase 3: installed — show main dashboard
  return <Dashboard />;
}
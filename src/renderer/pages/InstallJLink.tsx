import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { useProbeStore } from "../store/probeStore";
import {
  COMMANDS, EVENTS,
  InstallResult, ScanInstallerResult, DownloadProgress, ArchInfo,
} from "@shared/types";

type Phase =
  | 'checking'
  | 'no-installer'
  | 'has-installer'
  | 'downloading'
  | 'installing'
  | 'error';

// UA-based platform detection used as an immediate fallback until get_arch_info resolves.
const uaPlatform = (): "win32" | "darwin" | "linux" => {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("win")) return "win32";
  if (ua.includes("mac")) return "darwin";
  return "linux";
};

const PLATFORM_COPY = {
  win32:  { elevationNote: "A UAC prompt will appear — click Yes to allow installation.", installingNote: "Installing silently in the background...", downloadBtnLabel: "⬇️ Download & Install J-Link Software", installBtnLabel: "🛠️ Install J-Link Software" },
  darwin: { elevationNote: "An administrator password prompt will appear.", installingNote: "Running installer — this may take a minute...", downloadBtnLabel: "⬇️ Download & Install J-Link Software", installBtnLabel: "🛠️ Install J-Link Package" },
  linux:  { elevationNote: "A privilege prompt (pkexec) will appear.", installingNote: "Running dpkg installer — this may take a minute...", downloadBtnLabel: "⬇️ Download & Install J-Link Software", installBtnLabel: "🛠️ Install J-Link Package" },
};

/** Builds an arch-specific installer label using info returned by the Rust backend. */
function getInstallerLabel(archInfo: ArchInfo | null): string {
  if (!archInfo) return "J-Link Software Installer";
  const { os, arch } = archInfo;
  if (os === "windows") {
    if (arch === "aarch64") return "J-Link Windows Installer (ARM64, .exe)";
    if (arch === "x86")     return "J-Link Windows Installer (32-bit, .exe)";
    return "J-Link Windows Installer (64-bit, .exe)";
  }
  if (os === "macos") {
    if (arch === "aarch64") return "J-Link macOS Package (Apple Silicon, .pkg)";
    if (arch === "x86_64")  return "J-Link macOS Package (Intel, .pkg)";
    return "J-Link macOS Package (Universal, .pkg)";
  }
  // Linux
  if (arch === "aarch64") return "J-Link Linux Package (ARM64, .deb)";
  if (arch === "arm")     return "J-Link Linux Package (ARM 32-bit, .deb)";
  if (arch === "x86")     return "J-Link Linux Package (32-bit, .deb)";
  return "J-Link Linux Package (64-bit, .deb)";
}

export default function InstallJLink() {
  const { checkInstallation, isLoading } = useProbeStore();

  const [phase, setPhase]                       = useState<Phase>('checking');
  const [installerPath, setInstallerPath]        = useState<string>('');
  const [downloadProgress, setDownloadProgress] = useState<number>(0);
  const [progressLabel, setProgressLabel]        = useState<string>('');
  const [statusMessage, setStatusMessage]        = useState<string>('');
  const [errorMessage, setErrorMessage]          = useState<string>('');
  const [archInfo, setArchInfo]                  = useState<ArchInfo | null>(null);

  // Derive platform and copy from live arch info; fall back to UA until the command resolves.
  const platform = archInfo
    ? (archInfo.os === "windows" ? "win32" : archInfo.os === "macos" ? "darwin" : "linux")
    : uaPlatform();
  const copy           = PLATFORM_COPY[platform] ?? PLATFORM_COPY.win32;
  const installerLabel = getInstallerLabel(archInfo);

  // Keep refs to all active listeners so we can cancel them
  const listenersRef = useRef<UnlistenFn[]>([]);
  const installingRef = useRef<boolean>(false);

  const cleanupListeners = () => {
    listenersRef.current.forEach(fn => fn());
    listenersRef.current = [];
  };

  useEffect(() => {
    // Fetch arch info and installer scan in parallel
    invoke<ArchInfo>(COMMANDS.GET_ARCH_INFO)
      .then(setArchInfo)
      .catch(() => { /* leave archInfo null — UA fallback stays active */ });

    (async () => {
      try {
        const result = await invoke<ScanInstallerResult>(COMMANDS.SCAN_FOR_INSTALLER);
        if (result.found) {
          setInstallerPath(result.path);
          setPhase('has-installer');
        } else {
          setPhase('no-installer');
        }
      } catch {
        setPhase('no-installer');
      }
    })();
    // Cleanup all listeners on unmount
    return () => cleanupListeners();
  }, []);

  // ── Install (shared logic) ────────────────────────────────────────────────────
  const runInstall = async (installerPath: string) => {
    if (installingRef.current) {
      console.warn('[install] Already installing, ignoring duplicate call');
      return;
    }
    installingRef.current = true;
    cleanupListeners();

    setPhase('installing');
    setStatusMessage(copy.elevationNote);
    try {
      const result = await invoke<InstallResult>(COMMANDS.INSTALL_JLINK, { installerPath });
      if (result.success) {
        setTimeout(() => checkInstallation(), 1500);
      } else if (result.cancelled) {
        setPhase('has-installer');
        setStatusMessage('');
      } else {
        setErrorMessage(result.message);
        setPhase('error');
      }
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setPhase('error');
    } finally {
      installingRef.current = false;
    }
  };

  // ── Download & Install ────────────────────────────────────────────────────────
  const handleDownloadAndInstall = async () => {
    // Cancel any previous listeners before starting new download
    cleanupListeners();

    setPhase('downloading');
    setDownloadProgress(0);
    setProgressLabel('Opening SEGGER download page...');

    try {
      const unlistenProgress = await listen<DownloadProgress>(EVENTS.DOWNLOAD_PROGRESS, (e) => {
        const { percent, transferred, total } = e.payload;
        const safePercent = Math.max(0, Math.min(100, percent));
        setDownloadProgress(safePercent);
        if (percent === 0) {
          // Reset — retry in progress
          setProgressLabel('');
        } else if (total > 0) {
          const mb = (transferred / 1024 / 1024).toFixed(1);
          const totalMb = Math.round(total / 1024 / 1024);
          setProgressLabel(`${safePercent}%  —  ${mb} MB / ${totalMb} MB`);
        } else {
          setProgressLabel(`${safePercent}%`);
        }
      });
      listenersRef.current.push(unlistenProgress);

      const unlistenCompleted = await listen<string>(EVENTS.DOWNLOAD_COMPLETED, async (e) => {
        // Only handle once — cleanup immediately
        cleanupListeners();
        const dlPath = e.payload;
        if (!dlPath) return;
        setInstallerPath(dlPath);
        await runInstall(dlPath);
      });
      listenersRef.current.push(unlistenCompleted);

      const unlistenCancelled = await listen(EVENTS.DOWNLOAD_CANCELLED, () => {
        cleanupListeners();
        setPhase('no-installer');
        setProgressLabel('');
        setDownloadProgress(0);
      });
      listenersRef.current.push(unlistenCancelled);

      const unlistenSessionFailed = await listen(EVENTS.DOWNLOAD_SESSION_FAILED, () => {
        cleanupListeners();
        setPhase('no-installer');
        setProgressLabel('');
        setDownloadProgress(0);
        setErrorMessage('Download session failed. Please restart the application and try again.');
      });
      listenersRef.current.push(unlistenSessionFailed);

      const unlistenRetrying = await listen(EVENTS.DOWNLOAD_RETRYING, () => {
        // Reset progress immediately when retry starts — before new animation begins
        setDownloadProgress(0);
        setProgressLabel('Retrying...');
      });
      listenersRef.current.push(unlistenRetrying);

      const unlistenRetry = await listen(EVENTS.DOWNLOAD_RETRY, () => {
        cleanupListeners();
        setDownloadProgress(0);
        setProgressLabel('Retrying...');
        setTimeout(() => handleDownloadAndInstall(), 600);
      });
      listenersRef.current.push(unlistenRetry);

      const dlResult = await invoke<{ success: boolean; path: string; mode?: string }>(
        COMMANDS.DOWNLOAD_JLINK
      );

      if (!dlResult.success) {
        cleanupListeners();
        setErrorMessage('Failed to open download page');
        setPhase('error');
        return;
      }

      if (dlResult.mode === 'webview-intercept' || dlResult.mode === 'webview-fetch') {
        setProgressLabel('Downloading from SEGGER...');
        // Listeners handle the rest
        return;
      }

      if (dlResult.mode === 'winget') {
        cleanupListeners();
        setTimeout(() => checkInstallation(), 1500);
        return;
      }

    } catch (err) {
      cleanupListeners();
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setPhase('error');
    }
  };

  // ── Install only (has existing installer) ────────────────────────────────────
  const handleInstallOnly = async () => {
    if (!installerPath) return;
    await runInstall(installerPath);
  };

  // ── Cancel ───────────────────────────────────────────────────────────────────
  const handleCancel = async () => {
    if (phase === 'downloading') {
      await invoke(COMMANDS.CANCEL_DOWNLOAD);
    } else if (phase === 'installing') {
      installingRef.current = false;
      setPhase('has-installer');
      setStatusMessage('');
      await invoke(COMMANDS.CANCEL_INSTALL, { keepInstaller: true });
    }
  };

  const isBusy = phase === 'downloading' || phase === 'installing';

  return (
    <div className="container">
      <div className="not-installed-message">
        <div className="message-card">
          <h2>J-Link Software Not Found</h2>
          <p>SEGGER J-Link Software is required to use this application.</p>

          <div style={{ marginTop: '8px', fontSize: '13px', color: '#6c757d' }}>
            {platform === 'win32'  && `🪟 Windows — will download ${installerLabel}`}
            {platform === 'darwin' && `🍎 macOS — will download ${installerLabel}`}
            {platform === 'linux'  && `🐧 Linux — will download ${installerLabel}`}
          </div>

          <div style={{ marginTop: '24px' }}>

            {phase === 'checking' && (
              <div style={{ color: '#6c757d', fontSize: '13px' }}>🔍 Checking for existing installer...</div>
            )}

            {phase === 'downloading' && (
              <div style={{ marginBottom: '20px' }}>
                <div style={{ fontWeight: 600, marginBottom: '8px', color: '#495057' }}>📥 Downloading J-Link...</div>
                <div style={{
                  width: '100%', height: '8px',
                  backgroundColor: '#e9ecef', borderRadius: '4px',
                  overflow: 'hidden', marginBottom: '8px',
                }}>
                  <div style={{
                    width: downloadProgress > 0 ? `${downloadProgress}%` : '100%',
                    height: '100%',
                    backgroundColor: '#007bff',
                    borderRadius: '4px',
                    transition: downloadProgress > 0 ? 'width 0.3s ease' : 'none',
                    opacity: downloadProgress === 0 ? 0.4 : 1,
                    animation: downloadProgress === 0 ? 'pulse 1.5s ease-in-out infinite' : 'none',
                  }} />
                </div>
                <style>{`@keyframes pulse { 0%,100%{opacity:0.3} 50%{opacity:0.7} }`}</style>
                <div style={{ fontSize: '13px', color: '#6c757d' }}>{progressLabel}</div>
              </div>
            )}

            {phase === 'installing' && (
              <div style={{ marginBottom: '20px', padding: '14px', backgroundColor: '#f8f9fa', borderRadius: '8px', border: '1px solid #e9ecef' }}>
                <div style={{ fontWeight: 600, marginBottom: '6px', color: '#495057' }}>⚙️ Installing J-Link...</div>
                <div style={{ fontSize: '13px', color: '#6c757d', marginBottom: '4px' }}>{statusMessage}</div>
                <div style={{ fontSize: '13px', color: '#adb5bd' }}>{copy.installingNote}</div>
              </div>
            )}

            {phase === 'error' && (
              <div style={{ marginBottom: '20px', padding: '14px', backgroundColor: '#fff0f0', borderRadius: '8px', border: '1px solid #ffcccc' }}>
                <div style={{ fontWeight: 600, marginBottom: '6px', color: '#721c24' }}>❌ Error</div>
                <div style={{ fontSize: '13px', color: '#721c24', marginBottom: '8px' }}>{errorMessage}</div>
                <div style={{ fontSize: '13px', color: '#6c757d' }}>
                  You can also install J-Link manually from{' '}
                  <a href="#" onClick={(e) => { e.preventDefault(); open('https://www.segger.com/downloads/jlink/'); }}
                    style={{ color: '#007bff', cursor: 'pointer' }}>
                    segger.com/downloads/jlink
                  </a>
                </div>
              </div>
            )}

            <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
              {(phase === 'no-installer' || phase === 'downloading') && (
                <button className="btn btn-primary" onClick={handleDownloadAndInstall}
                  disabled={isBusy || isLoading} style={{ flex: 1, minWidth: '260px' }}>
                  {phase === 'downloading' ? '⏳ Downloading...' : copy.downloadBtnLabel}
                </button>
              )}

              {(phase === 'has-installer' || phase === 'installing') && (
                <button className="btn btn-primary" onClick={handleInstallOnly}
                  disabled={isBusy || isLoading} style={{ flex: 1, minWidth: '260px' }}>
                  {phase === 'installing' ? '⚙️ Installing...' : copy.installBtnLabel}
                </button>
              )}

              {phase === 'error' && (
                <button className="btn btn-secondary"
                  onClick={() => { setPhase('no-installer'); setErrorMessage(''); }}
                  style={{ flex: 1, minWidth: '160px' }}>
                  🔄 Try Again
                </button>
              )}

              {isBusy && (
                <button className="btn btn-danger" onClick={handleCancel}
                  style={{ minWidth: '100px' }}>
                  ✕ Cancel
                </button>
              )}
            </div>

            {!isBusy && phase !== 'checking' && (
              <div style={{ marginTop: '16px', fontSize: '13px', color: '#adb5bd', textAlign: 'center' }}>
                Prefer to install manually?{' '}
                <a href="#" onClick={(e) => { e.preventDefault(); open('https://www.segger.com/downloads/jlink/'); }}
                  style={{ color: '#6c757d', cursor: 'pointer' }}>
                  Download from SEGGER
                </a>
              </div>
            )}

          </div>
        </div>
      </div>
    </div>
  );
}
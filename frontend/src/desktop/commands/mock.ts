// Extensions spelled out: `node --test` runs this file directly, without a bundler.
import type { DesktopErrorName } from '../../types/errors.ts';
import { DesktopError } from './errors.ts';
import type { AppInfo, DesktopCommands, DesktopOs, ExportAvailability, ExportUse, LicenseTextId, Licenses, ModelBundle, ModelBundleId, ModelStatus, ModelTransfer, SaveAsResult, TransferErrorName } from './types.ts';
import { installNeedsBytes } from '../utils/modelSetup.ts';

const MB = 1_000_000;
const GB = 1_000 * MB;

/** The two bundles of spec §9.1, sizes rounded to the manifest's. */
const BUNDLES: readonly Omit<ModelBundle, 'installed' | 'active' | 'usedByExports'>[] = [
  { id: 'kokoro-en-v1_0-fp32', variant: 'fp32', displayName: 'Full quality', downloadBytes: 350 * MB, installedBytes: 384 * MB },
  { id: 'kokoro-en-v1_0-int8', variant: 'int8', displayName: 'Smaller', downloadBytes: 132 * MB, installedBytes: 173 * MB },
];
const IMPORTED_BUNDLE: ModelBundleId = 'kokoro-en-v1_0-int8';

const STARTING_MS = 3000;
const TICK_MS = 250;
/** The clock moves faster than the speed it reports, so a flow finishes in seconds. */
const DOWNLOAD_BYTES_PER_TICK = 25 * MB * (TICK_MS / 1000);
const SHOWN_BYTES_PER_SECOND = 2.1 * MB;
const VERIFY_STAGE_MS = 1500;
const ENGINE_LOAD_MS = 1500;
const MOVE_MS = 1500;

const SAVED_PATH: Record<DesktopOs, string> = { linux: '/home/sam/Music/Audiobook.m4b', windows: 'C:\\Users\\Sam\\Music\\Audiobook.m4b' };
/** Invented, like every title in the mocks. */
const EXPORT_IN_USE: ExportUse = { exportId: 'export-1', title: 'A Field Guide to Small Gardens' };

export type MockImportError = Extract<TransferErrorName, 'ModelChecksumMismatch' | 'FileUnreadable'>;
export type MockSaveAsError = Extract<DesktopErrorName, 'StorageFull' | 'StorageFailed'>;
export type MockRevealError = Extract<DesktopErrorName, 'ExportNotFound' | 'Internal'>;

/** The registry's codes and messages for the rejections the mock can give (core `error.rs`, shell `Internal`). */
const REJECTIONS: Record<MockSaveAsError | MockRevealError, { code: number; message: string }> = {
  StorageFull: { code: 4004, message: "There isn't enough free space." },
  StorageFailed: { code: 9001, message: 'Something went wrong saving data.' },
  ExportNotFound: { code: 3001, message: 'Export not found.' },
  Internal: { code: 9999, message: 'Something went wrong.' },
};
const reject = (name: MockSaveAsError | MockRevealError): DesktopError => new DesktopError(name, REJECTIONS[name].code, REJECTIONS[name].message);

const LICENSE_TEXT_IDS: readonly LicenseTextId[] = ['lectrice', 'kokoro', 'sherpaOnnx', 'espeakNg', 'onnxruntime', 'pdfium', 'ffmpeg', 'tauri', 'react'];
const MOCK_LICENSE_TEXT = 'The full license text is shown here.';
const MOCK_OTHER_PACKAGES = 412;

/** Like the real bundles: FFmpeg's license ships only with the Windows app. */
const mockLicenses = (os: DesktopOs): Licenses => ({
  texts: Object.fromEntries(LICENSE_TEXT_IDS.filter((id) => id !== 'ffmpeg' || os === 'windows').map((id) => [id, MOCK_LICENSE_TEXT])),
  otherPackages: MOCK_OTHER_PACKAGES,
});

export interface MockDesktopOptions {
  os?: DesktopOs;
  freeBytes?: number;
  exportAvailability?: ExportAvailability;
  /** A bundle an unfinished export needs: deleting it is refused. */
  bundleUsedByExport?: ModelBundleId | null;
  /** How an import ends after its check: installed (null), or failed with this error (`ModelChecksumMismatch` = not a voice model). */
  importError?: MockImportError | null;
  /** "Save as…" rejects with this name instead of saving. */
  saveAsError?: MockSaveAsError | null;
  /** "Show in folder" rejects with this name (`ExportNotFound` = the file is gone, `Internal` = no file manager). */
  revealError?: MockRevealError | null;
  /** Multiplies every delay; tests pass a small number. */
  timeScale?: number;
}

/** A desktop shell that exists only in memory: the download, import, switch, delete and save flows, with timers and the chosen failures. */
export const createMockDesktopCommands = ({ os = 'linux', freeBytes = 23.4 * GB, exportAvailability = { available: true }, bundleUsedByExport = null, importError = null, saveAsError = null, revealError = null, timeScale = 1 }: MockDesktopOptions = {}): DesktopCommands => {
  const listeners = new Set<(status: ModelStatus) => void>();
  const usedBy = (id: ModelBundleId): ExportUse[] => (id === bundleUsedByExport ? [EXPORT_IN_USE] : []);
  let status: ModelStatus = {
    bundles: BUNDLES.map((bundle) => ({ ...bundle, installed: false, active: false, usedByExports: usedBy(bundle.id) })),
    engine: 'missing',
    transfer: { kind: 'none' },
    freeBytes,
    setupDismissed: false,
  };
  let timer: ReturnType<typeof setTimeout> | null = null;

  const update = (patch: Partial<ModelStatus>) => {
    status = { ...status, ...patch };
    listeners.forEach((listener) => listener(status));
  };
  const setTransfer = (transfer: ModelTransfer) => update({ transfer });
  const bundleOf = (id: ModelBundleId): ModelBundle => status.bundles.find((bundle) => bundle.id === id)!;
  const patchBundle = (id: ModelBundleId, patch: Partial<ModelBundle>) => update({ bundles: status.bundles.map((bundle) => (bundle.id === id ? { ...bundle, ...patch } : bundle)) });

  const stopClock = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  const after = (ms: number, step: () => void) => {
    stopClock();
    timer = setTimeout(step, ms * timeScale);
  };

  const loadEngine = () => {
    update({ engine: 'loading' });
    after(ENGINE_LOAD_MS, () => update({ engine: 'ready' }));
  };
  const install = (id: ModelBundleId) => {
    const activate = !status.bundles.some((bundle) => bundle.active);
    patchBundle(id, { installed: true, active: activate || bundleOf(id).active });
    setTransfer({ kind: 'none' });
    if (activate) loadEngine();
  };
  const verify = (id: ModelBundleId, source: 'download' | 'import') => {
    setTransfer({ kind: 'verifying', bundleId: id, source, stage: 'checking' });
    after(VERIFY_STAGE_MS, () => {
      setTransfer({ kind: 'verifying', bundleId: id, source, stage: 'unpacking' });
      after(VERIFY_STAGE_MS, () => install(id));
    });
  };
  const download = (id: ModelBundleId, doneBytes: number) => {
    const totalBytes = bundleOf(id).downloadBytes;
    if (doneBytes >= totalBytes) {
      verify(id, 'download');
      return;
    }
    setTransfer({ kind: 'downloading', bundleId: id, doneBytes, totalBytes, bytesPerSecond: SHOWN_BYTES_PER_SECOND });
    after(TICK_MS, () => download(id, Math.min(totalBytes, doneBytes + DOWNLOAD_BYTES_PER_TICK)));
  };

  const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms * timeScale));

  return {
    appInfo: async (): Promise<AppInfo> => ({ appVersion: '0.1.0', coreVersion: '0.3.0', os, arch: 'x64' }),

    licenses: async () => mockLicenses(os),

    modelStatus: async () => status,

    onModelStatus: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    async modelDownloadStart(id) {
      const transfer = status.transfer;
      if (transfer.kind === 'paused' && transfer.bundleId === id) {
        download(id, transfer.doneBytes);
        return;
      }
      if (status.freeBytes !== null && status.freeBytes < installNeedsBytes(bundleOf(id))) {
        setTransfer({ kind: 'failed', bundleId: id, source: 'download', error: 'StorageFull' });
        return;
      }
      setTransfer({ kind: 'starting', bundleId: id });
      after(STARTING_MS, () => download(id, 0));
    },

    async modelDownloadPause() {
      const transfer = status.transfer;
      if (transfer.kind !== 'downloading' && transfer.kind !== 'retrying' && transfer.kind !== 'waitingForNetwork') return;
      stopClock();
      setTransfer({ kind: 'paused', bundleId: transfer.bundleId, doneBytes: transfer.doneBytes, totalBytes: transfer.totalBytes });
    },

    async modelDownloadCancel() {
      stopClock();
      setTransfer({ kind: 'none' });
    },

    /** Both kinds run the same way: the shell checks, then unpacks the archive or copies the folder's files. */
    async modelImport() {
      setTransfer({ kind: 'verifying', bundleId: null, source: 'import', stage: 'checking' });
      after(VERIFY_STAGE_MS, () => {
        if (importError !== null) {
          setTransfer({ kind: 'failed', bundleId: null, source: 'import', error: importError });
          return;
        }
        setTransfer({ kind: 'verifying', bundleId: IMPORTED_BUNDLE, source: 'import', stage: 'unpacking' });
        after(VERIFY_STAGE_MS, () => install(IMPORTED_BUNDLE));
      });
      return true;
    },

    async modelActivate(id) {
      update({ bundles: status.bundles.map((bundle) => ({ ...bundle, active: bundle.id === id })) });
      loadEngine();
    },

    async modelDelete(id) {
      const blockingExports = bundleOf(id).usedByExports;
      if (blockingExports.length > 0) return { deleted: false, blockingExports };
      patchBundle(id, { installed: false });
      return { deleted: true, blockingExports: [] };
    },

    async modelSetupDismiss() {
      update({ setupDismissed: true });
    },

    exportList: async () => [],

    async exportSaveAs(): Promise<SaveAsResult> {
      await wait(MOVE_MS);
      if (saveAsError !== null) throw reject(saveAsError);
      return { outcome: 'saved', path: SAVED_PATH[os] };
    },

    async exportReveal() {
      if (revealError !== null) throw reject(revealError);
    },

    exportAvailability: async () => exportAvailability,

    openLink: async () => {},
  };
};

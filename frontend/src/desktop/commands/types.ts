import type { AudioExport } from '../../types/api';
import type { DesktopErrorName } from '../../types/errors';

/*
 * The desktop shell's own commands (desktop spec §4.4), as the UI sees them. One implementation is the mock in
 * `./mock.ts`; the Tauri adapter (`./tauri.ts`) implements the same interface over `invoke` and the `model-status`
 * event, translating the shell's wire format (`./wire.ts`, contract in lectrice-desktop's
 * `docs/2026-10-24-desktop-model-contract.md`). Every rejection is a `DesktopError` (`./errors.ts`) carrying one of
 * the registry names.
 */

export type ModelBundleId = 'kokoro-en-v1_0-fp32' | 'kokoro-en-v1_0-int8';
export type ModelVariant = 'fp32' | 'int8';

/** An unfinished export made with a bundle's engine. */
export interface ExportUse { exportId: string; title: string; }

export interface ModelBundle {
  id: ModelBundleId;
  variant: ModelVariant;
  /** From the manifest: "Full quality", "Smaller". */
  displayName: string;
  downloadBytes: number;
  installedBytes: number;
  installed: boolean;
  /** The bundle the engine uses (or is loading). At most one. */
  active: boolean;
  /** Switching away asks first while this isn't empty; deleting is refused. */
  usedByExports: readonly ExportUse[];
}

export type EngineStatus = 'missing' | 'loading' | 'ready' | 'failed';

export type TransferSource = 'download' | 'import';

export type TransferErrorName = Extract<DesktopErrorName, 'ModelDownloadFailed' | 'ModelChecksumMismatch' | 'StorageFull' | 'FileUnreadable'>;

interface TransferProgress { bundleId: ModelBundleId; doneBytes: number; totalBytes: number; }

/** The one download or file import that may be under way (spec §9.2). An import's bundle is known once its hash matched. */
export type ModelTransfer =
  | { kind: 'none' }
  | { kind: 'starting'; bundleId: ModelBundleId }
  | ({ kind: 'downloading'; bytesPerSecond: number | null } & TransferProgress)
  /** The first damaged part is downloaded again; the second failure is `failed` with ModelChecksumMismatch. */
  | ({ kind: 'retrying' } & TransferProgress)
  | ({ kind: 'paused' } & TransferProgress)
  | ({ kind: 'waitingForNetwork'; retryInSeconds: number } & TransferProgress)
  | { kind: 'verifying'; bundleId: ModelBundleId | null; source: TransferSource; stage: 'checking' | 'unpacking' }
  | { kind: 'failed'; bundleId: ModelBundleId | null; source: TransferSource; error: TransferErrorName };

export interface ModelStatus {
  bundles: readonly ModelBundle[];
  engine: EngineStatus;
  transfer: ModelTransfer;
  /** Free space on the drive that holds the models; null when the system can't tell. */
  freeBytes: number | null;
  /** "Not now, just read" was chosen (kept across restarts). */
  setupDismissed: boolean;
}

/** `model_delete` refuses a bundle unfinished exports use, and names them. */
export interface ModelDeleteResult { deleted: boolean; blockingExports: readonly ExportUse[]; }

/** "Import from file…" picks a downloaded archive; a folder is an unpacked bundle. */
export type ModelImportKind = 'archive' | 'folder';

export type DesktopOs = 'linux' | 'windows';

export interface AppInfo {
  appVersion: string;
  coreVersion: string;
  os: DesktopOs;
  /** "x64" */
  arch: string;
}

export type ExportUnavailableReason = 'ffmpegMissing' | 'ffmpegCannotMakeM4b' | 'bundledFfmpegMissing';

export type ExportAvailability = { available: true } | { available: false; reason: ExportUnavailableReason };

/** The native "Save as" dialog, then the move. `cancelled` when the dialog was closed without a choice. */
export type SaveAsResult = { outcome: 'saved'; path: string } | { outcome: 'cancelled' };

/** The fixed list `open_link` accepts; never a free-form URL. */
export type LinkId = 'releases' | 'source' | 'modelDownloads' | 'thirdPartyLicenses';

export type LicenseTextId = 'lectrice' | 'kokoro' | 'sherpaOnnx' | 'espeakNg' | 'onnxruntime' | 'pdfium' | 'ffmpeg' | 'tauri' | 'react';

/** The license texts the app ships. A text this build doesn't carry (FFmpeg on Linux) is missing. */
export interface Licenses {
  texts: Partial<Record<LicenseTextId, string>>;
  /** The Rust crates and npm packages in the bundled full list; null when the build has no list. */
  otherPackages: number | null;
}

export interface DesktopCommands {
  appInfo(): Promise<AppInfo>;
  licenses(): Promise<Licenses>;
  modelStatus(): Promise<ModelStatus>;
  /** Status changes (the `model-status` event, at most 4 per second). Returns the unsubscribe. */
  onModelStatus(listener: (status: ModelStatus) => void): () => void;
  /** Starts, resumes or retries the download of one bundle. */
  modelDownloadStart(bundleId: ModelBundleId): Promise<void>;
  modelDownloadPause(): Promise<void>;
  /** Also deletes the partial download. */
  modelDownloadCancel(): Promise<void>;
  /**
   * Opens the native file (or folder) dialog on the Rust side; progress and failures come through `onModelStatus`.
   * False when the dialog was cancelled.
   */
  modelImport(kind: ModelImportKind): Promise<boolean>;
  /** Makes the bundle the engine's (engine reloads); also the retry after `engine: 'failed'`. */
  modelActivate(bundleId: ModelBundleId): Promise<void>;
  /** Not deleted while an unfinished export needs this bundle. */
  modelDelete(bundleId: ModelBundleId): Promise<ModelDeleteResult>;
  /** "Not now, just read". */
  modelSetupDismiss(): Promise<void>;
  /** Every book's exports, newest first (the "ready" toast). */
  exportList(): Promise<AudioExport[]>;
  /** Moves the finished file where the user picks; rejects with `StorageFull` or `StorageFailed`. */
  exportSaveAs(exportId: string): Promise<SaveAsResult>;
  /** Rejects with `ExportNotFound` when the file was moved or deleted outside Lectrice. */
  exportReveal(exportId: string): Promise<void>;
  /** The ffmpeg check (spec §8.1); while it fails, every call looks again ("Check again"). */
  exportAvailability(): Promise<ExportAvailability>;
  openLink(linkId: LinkId): Promise<void>;
}

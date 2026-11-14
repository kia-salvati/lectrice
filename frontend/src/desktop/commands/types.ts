import type { AudioExport } from '../../types/api';
import type { DesktopErrorName } from '../../types/errors';

/*
 * The desktop shell's own commands, as the UI sees them. The Tauri adapter (`./tauri.ts`) implements them over
 * `invoke` and the `model-status` event, translating the shell's wire format (`./wire.ts`, contract in
 * lectrice-desktop's `docs/2026-10-24-desktop-model-contract.md`); the tests use an in-memory fake in `tests/mocks/`.
 * Every rejection is a `DesktopError` (`./errors.ts`) carrying one of
 * the registry names.
 */

/** A bundle id from the shell's manifest. */
export type ModelBundleId = string;
/** `int8` is the retired official Smaller model: the shell lists it only while it is installed or partly downloaded. */
export type ModelVariant = 'fast' | 'fp32' | 'small' | 'int8';

/** An unfinished export made with a bundle's engine. */
export interface ExportUse { exportId: string; title: string; }

export interface ModelBundle {
  id: ModelBundleId;
  variant: ModelVariant;
  /** From the manifest: "Fast", "Full quality", "Small". */
  displayName: string;
  /** The download's file name, e.g. "….tar.bz2". */
  archiveName: string;
  /** The folder an unpacked bundle has. */
  folderName: string;
  voiceCount: number;
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

/*
 * "Let my phone use this computer" (lectrice-desktop `docs/2026-11-13-phone-helper-spec.md`, UX brief
 * `docs/2026-11-13-phone-helper-ux-brief.md` §3). The shell sends these shapes as they are (event `phone-helper`).
 */

export type PhoneHelperListening = 'off' | 'starting' | 'ready' | 'failed';

/** Why a phone may not reach this computer; null when nothing is known to be wrong. */
export type PhoneHelperNotice =
  | { kind: 'windowsFirstListen' }
  | { kind: 'linuxFirewall'; commands: readonly string[] }
  | { kind: 'noNetwork' }
  | { kind: 'portInUse'; port: number };

/** The helper listens only on networks the user allowed. */
export type PhoneHelperNetwork = { kind: 'allowed'; name: string | null } | { kind: 'other'; name: string | null } | { kind: 'none' };

export interface PairedPhone {
  /** 16 hex digits, stable. */
  peerId: string;
  name: string;
  state: 'making' | 'connected' | 'offline';
  /** ISO time of the last session; null if it never connected since pairing. */
  lastSeenAt: string | null;
}

/** A wrong code never brings a new code by itself. */
export type PhonePairing =
  | { kind: 'none' }
  | { kind: 'showing'; code: string; expiresAt: number; totalSeconds: number; wrongCodes: number; trying: readonly string[] }
  | { kind: 'paired'; phoneName: string; peerId: string }
  | { kind: 'expired' }
  | { kind: 'tooManyWrongCodes'; lastFrom: string | null }
  | { kind: 'coolingDown'; until: number };

export interface PhoneModelRequest { bundleId: ModelBundleId; displayName: string; downloadBytes: number; phoneName: string; }

export interface PhoneHelperStatus {
  enabled: boolean;
  listening: PhoneHelperListening;
  computerName: string;
  /** "192.168.1.20:47878"; the first is the one to show. Empty while off. */
  addresses: readonly string[];
  notice: PhoneHelperNotice | null;
  network: PhoneHelperNetwork;
  phones: readonly PairedPhone[];
  pairing: PhonePairing;
  /** A phone asked for a voice this computer doesn't have. */
  modelRequest: PhoneModelRequest | null;
  versionMismatch: { update: 'computer' | 'phone'; phoneName: string } | null;
  downloadVoicesForPhone: boolean;
  port: number;
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
  phoneHelperStatus(): Promise<PhoneHelperStatus>;
  /** Status changes (the `phone-helper` event, at most 2 per second). Returns the unsubscribe. */
  onPhoneHelperStatus(listener: (status: PhoneHelperStatus) => void): () => void;
  phoneHelperSetEnabled(enabled: boolean): Promise<void>;
  /** "Pair a phone", also "New code". */
  phoneHelperPairStart(): Promise<void>;
  phoneHelperPairCancel(): Promise<void>;
  phoneHelperForget(peerId: string): Promise<void>;
  /** This computer's name: 1–40 characters, trimmed. */
  phoneHelperRename(name: string): Promise<void>;
  phoneHelperSetDownloadVoices(on: boolean): Promise<void>;
  /** 1024–65535. */
  phoneHelperSetPort(port: number): Promise<void>;
  phoneHelperDismissModelRequest(): Promise<void>;
  phoneHelperDismissVersionMismatch(): Promise<void>;
  /** "Use on this network too". */
  phoneHelperAllowThisNetwork(): Promise<void>;
}

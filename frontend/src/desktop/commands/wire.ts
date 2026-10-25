// Extensions spelled out: `node --test` runs this file directly, without a bundler.
import type { DesktopErrorName } from '../../types/errors.ts';
import { DesktopError } from './errors.ts';
import type { DesktopOs, ExportAvailability, ExportUse, ModelBundleId, ModelStatus, ModelTransfer, ModelVariant, TransferErrorName, TransferSource } from './types.ts';

/*
 * What the desktop shell sends over IPC (lectrice-desktop's `docs/2026-10-24-desktop-model-contract.md`), and the
 * mapping to the UI's types. Kept apart from `./tauri.ts` so it can be tested without Tauri.
 */

export interface ErrorBodyDto { name: string; code: number; message: string; }

interface BundleDto {
  id: ModelBundleId;
  variant: ModelVariant;
  name: string;
  downloadBytes: number;
  installedBytes: number;
  installed: boolean;
  active: boolean;
  usedByExports: ExportUse[];
}

type EngineDto =
  | { state: 'missing' }
  | { state: 'loading' | 'ready'; bundleId: ModelBundleId | null }
  | { state: 'failed'; bundleId: ModelBundleId | null; error: ErrorBodyDto };

interface ProgressDto { bundleId: ModelBundleId; bytesDone: number; bytesTotal: number; }

type DownloadDto =
  | { state: 'idle' }
  | ({ state: 'starting' } & ProgressDto)
  | ({ state: 'downloading'; bytesPerSecond: number | null; secondsLeft: number | null; retrying: boolean } & ProgressDto)
  | ({ state: 'paused' } & ProgressDto)
  | ({ state: 'waitingForNetwork'; retryInSeconds: number } & ProgressDto)
  | { state: 'checking'; bundleId: ModelBundleId | null; source: TransferSource }
  | { state: 'unpacking'; bundleId: ModelBundleId; source: TransferSource }
  | { state: 'failed'; bundleId: ModelBundleId | null; source: TransferSource; error: ErrorBodyDto }
  | { state: 'noSpace'; bundleId: ModelBundleId; source: TransferSource; neededBytes: number; freeBytes: number };

export interface ModelStatusDto {
  bundles: BundleDto[];
  engine: EngineDto;
  download: DownloadDto;
  freeBytes: number | null;
  setupDismissed: boolean;
}

export interface ExportAvailabilityDto { available: boolean; problem: 'notFound' | 'notRunnable' | 'missingComponents' | null; }

const SHOWN_ERRORS: readonly TransferErrorName[] = ['ModelDownloadFailed', 'ModelChecksumMismatch', 'StorageFull', 'FileUnreadable'];
/** What the screens say for an error they have no line of their own for (a write that failed, an internal error). */
const FALLBACK_ERROR: Record<TransferSource, TransferErrorName> = { download: 'ModelDownloadFailed', import: 'FileUnreadable' };

const transferError = (name: string, source: TransferSource): TransferErrorName => SHOWN_ERRORS.find((shown) => shown === name) ?? FALLBACK_ERROR[source];

const toTransfer = (download: DownloadDto): ModelTransfer => {
  switch (download.state) {
    case 'idle':
      return { kind: 'none' };
    case 'starting':
      return { kind: 'starting', bundleId: download.bundleId };
    case 'downloading': {
      const progress = { bundleId: download.bundleId, doneBytes: download.bytesDone, totalBytes: download.bytesTotal };
      return download.retrying ? { kind: 'retrying', ...progress } : { kind: 'downloading', bytesPerSecond: download.bytesPerSecond, ...progress };
    }
    case 'paused':
      return { kind: 'paused', bundleId: download.bundleId, doneBytes: download.bytesDone, totalBytes: download.bytesTotal };
    case 'waitingForNetwork':
      return { kind: 'waitingForNetwork', retryInSeconds: download.retryInSeconds, bundleId: download.bundleId, doneBytes: download.bytesDone, totalBytes: download.bytesTotal };
    case 'checking':
    case 'unpacking':
      return { kind: 'verifying', bundleId: download.bundleId, source: download.source, stage: download.state };
    case 'failed':
      return { kind: 'failed', bundleId: download.bundleId, source: download.source, error: transferError(download.error.name, download.source) };
    case 'noSpace':
      return { kind: 'failed', bundleId: download.bundleId, source: download.source, error: 'StorageFull' };
  }
};

export const toModelStatus = (dto: ModelStatusDto): ModelStatus => ({
  bundles: dto.bundles.map((bundle) => ({
    id: bundle.id,
    variant: bundle.variant,
    displayName: bundle.name,
    downloadBytes: bundle.downloadBytes,
    installedBytes: bundle.installedBytes,
    installed: bundle.installed,
    active: bundle.active,
    usedByExports: bundle.usedByExports,
  })),
  engine: dto.engine.state,
  transfer: toTransfer(dto.download),
  // A refused install says how much is free even when the overall number is unknown.
  freeBytes: dto.freeBytes ?? (dto.download.state === 'noSpace' ? dto.download.freeBytes : null),
  setupDismissed: dto.setupDismissed,
});

/** Windows ships its own ffmpeg, so there a missing or broken one means a damaged install. */
export const toExportAvailability = (dto: ExportAvailabilityDto, os: DesktopOs): ExportAvailability => {
  if (dto.available || dto.problem === null) return { available: true };
  if (os === 'windows' && dto.problem !== 'missingComponents') return { available: false, reason: 'bundledFfmpegMissing' };
  return { available: false, reason: dto.problem === 'missingComponents' ? 'ffmpegCannotMakeM4b' : 'ffmpegMissing' };
};

const KNOWN_ERRORS: readonly DesktopErrorName[] = [
  'RequestInvalid', 'DocumentNotFound', 'SpeechEngineUnavailable', 'ExportNotFound', 'ExportNotReady', 'ExporterUnavailable', 'StorageFailed', 'Internal',
  'ModelMissing', 'ModelDownloadFailed', 'ModelChecksumMismatch', 'StorageFull', 'ExportPaused', 'FileUnreadable',
];

const isErrorBody = (value: unknown): value is { error: ErrorBodyDto } => {
  if (typeof value !== 'object' || value === null || !('error' in value)) return false;
  const inner: unknown = value.error;
  return typeof inner === 'object' && inner !== null && 'name' in inner && 'code' in inner && 'message' in inner && typeof inner.name === 'string' && typeof inner.code === 'number' && typeof inner.message === 'string';
};

/** A command's rejection (`{ error: { name, code, message } }`, or a string from Tauri itself) as a `DesktopError`. */
export const toDesktopError = (cause: unknown): DesktopError => {
  if (isErrorBody(cause)) {
    const name = KNOWN_ERRORS.find((known) => known === cause.error.name);
    if (name) return new DesktopError(name, cause.error.code, cause.error.message);
  }
  console.error('Desktop command failed', cause);
  return new DesktopError('Internal', 9999, 'Something went wrong.');
};

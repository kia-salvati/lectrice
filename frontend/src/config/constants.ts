import { platform } from '../platform';
import type { ExportFormat } from '../types/api';

/** Mirrors the backend's upload.max_mb. The server re-checks. */
export const MAX_UPLOAD_MB = 200;
export const MAX_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024;

export const PLAYBACK_SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2] as const;
export type PlaybackSpeed = (typeof PLAYBACK_SPEEDS)[number];
export const DEFAULT_SPEED: PlaybackSpeed = 1;

/** Sentences fetched ahead of the one playing. */
export const PREFETCH_AHEAD = 2;

/** Measured Kokoro rate at 1× (spec §4: ≈ 18 characters per second). Used only for "time left". */
export const SECONDS_PER_CHAR = 1 / 18;

/** Used until GET /api/voices answers, or if it fails. The server's defaultVoiceId wins once known. */
export const FALLBACK_VOICE_ID = 'af_heart';

export const POSITION_SAVE_DELAY_MS = 1500;

export const RECENT_PAGE_SIZE = 20;

export const EXPORT_FORMATS: readonly ExportFormat[] = platform.exportFormats;

export const EXPORT_FORMAT_LABEL: Record<ExportFormat, string> = { mp3: 'MP3', m4b: 'M4B (audiobook)' };

/** File extension per format, for the download name. */
export const EXPORT_FORMAT_EXTENSION: Record<ExportFormat, string> = { mp3: 'mp3', m4b: 'm4b' };

export const EXPORT_POLL_INTERVAL_MS = 1000;

/** The chapters rail needs this much width beside the text column; below it the bottom sheet is used. */
export const WIDE_SCREEN_QUERY = '(min-width: 1200px)';
/** Phone layout: the chapters sheet becomes a full-width modal. */
export const NARROW_SCREEN_QUERY = '(max-width: 720px)';

/** Whether the reader closed the chapters rail; remembered across books. */
export const RAIL_OPEN_STORAGE_KEY = 'lectrice.chaptersRailOpen';

/** How long the OS's file-dialog close animation gets before the screen under it changes (KWin's Scale effect ≈ 0.2 s). */
export const FILE_DIALOG_SETTLE_MS = 250;
/** The wait ends by then even if the window never reports focus again. */
export const FILE_DIALOG_SETTLE_MAX_MS = 1000;

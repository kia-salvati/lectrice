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

/** Formats offered for whole-book export, in menu order. The first is the default. */
export const EXPORT_FORMATS: readonly ExportFormat[] = ['mp3', 'm4b'];

export const EXPORT_FORMAT_LABEL: Record<ExportFormat, string> = { mp3: 'MP3', m4b: 'M4B (audiobook)' };

/** File extension per format, for the download name. */
export const EXPORT_FORMAT_EXTENSION: Record<ExportFormat, string> = { mp3: 'mp3', m4b: 'm4b' };

export const EXPORT_POLL_INTERVAL_MS = 1000;

import type { DocumentSummary } from './document';
import type { Voice } from './voice';

export interface ListResponse<T> { items: T[]; total: number; limit: number; offset: number; }

export interface PageRequest { limit: number; offset: number; }

export interface VoiceList { voices: Voice[]; defaultVoiceId: string; }

export interface ImportResult { document: DocumentSummary; alreadyInLibrary: boolean; }

export type SpeechEngineState = 'ready' | 'failed';

export interface Health { status: 'ok'; speechEngine: SpeechEngineState; }

export interface ApiErrorBody { error: { name: string; code: number; message: string; }; }

/** `paused` keeps the finished parts; starting the same export again continues it. */
export type ExportStatus = 'queued' | 'running' | 'paused' | 'done' | 'failed' | 'cancelled';

export type PauseReason = 'user' | 'system' | 'storageFull' | 'engineSwitch';

export interface ExportRange { fromSentence: number; toSentence: number; }

export type ExportLocation = { kind: 'appStorage'; path: string } | { kind: 'shared'; uri: string };

export interface AudioExport {
  id: string;
  documentId: string;
  voiceId: string;
  format: ExportFormat;
  /** The speed baked into the file: 75, 100, 125, 150, 175 or 200. */
  speedPercent: number;
  engineId: string;
  range: ExportRange | null;
  rangeLabel: string | null;
  /** The book's title when the export started. */
  title: string;
  status: ExportStatus;
  pausedBy: PauseReason | null;
  stage: 'parts' | 'assemble';
  /** A registry error name, e.g. "StorageFull". */
  error: string | null;
  sentencesDone: number;
  sentencesTotal: number;
  partsDone: number;
  partsTotal: number;
  estimatedSecondsLeft: number | null;
  fileBytes: number | null;
  location: ExportLocation | null;
  waitingForEngine: boolean;
}

export interface AudioExportResponse { export: AudioExport; }

/** GET /api/documents/{id}/exports, newest first. */
export interface AudioExportList { items: AudioExport[]; }

export type ExportFormat = 'mp3' | 'm4b';

export interface StartExportRequest { voiceId: string; format: ExportFormat; readSkipped: boolean; }

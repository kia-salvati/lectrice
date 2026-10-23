import type { DocumentSummary } from './document';
import type { Voice } from './voice';

export interface ListResponse<T> { items: T[]; total: number; limit: number; offset: number; }

export interface PageRequest { limit: number; offset: number; }

export interface VoiceList { voices: Voice[]; defaultVoiceId: string; }

export interface ImportResult { document: DocumentSummary; alreadyInLibrary: boolean; }

export type SpeechEngineState = 'ready' | 'failed';

export interface Health { status: 'ok'; speechEngine: SpeechEngineState; }

export interface ApiErrorBody { error: { name: string; code: number; message: string; }; }

export type ExportStatus = 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

export interface AudioExport {
  id: string;
  documentId: string;
  voiceId: string;
  format: ExportFormat;
  status: ExportStatus;
  sentencesDone: number;
  sentencesTotal: number;
  estimatedSecondsLeft: number | null;
}

export interface AudioExportResponse { export: AudioExport; }

/** GET /api/documents/{id}/exports, newest first. */
export interface AudioExportList { items: AudioExport[]; }

export type ExportFormat = 'mp3' | 'm4b';

export interface StartExportRequest { voiceId: string; format: ExportFormat; readSkipped: boolean; }

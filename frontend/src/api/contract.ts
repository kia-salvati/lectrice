import type { AudioExport, Health, ImportResult, ListResponse, PageRequest, StartExportRequest, VoiceList } from '../types/api';
import type { DocumentId, DocumentSummary, OpenedDocument } from '../types/document';

export interface SentenceAudioRequest {
  documentId: DocumentId;
  sentenceId: number;
  voiceId: string;
  /** The listener's speed. Audio is always made at 1×; the server only uses this to size its read-ahead. */
  rate: number;
  /** Whether read-ahead should include code, table and caption sentences. */
  readSkipped: boolean;
}

/** Both implementations (http and mock) satisfy this. Hooks only see this interface. */
export interface LectriceApi {
  getHealth(signal?: AbortSignal): Promise<Health>;
  listVoices(signal?: AbortSignal): Promise<VoiceList>;
  listDocuments(page: PageRequest, signal?: AbortSignal): Promise<ListResponse<DocumentSummary>>;
  importPdf(file: File, signal?: AbortSignal): Promise<ImportResult>;
  getDocument(id: DocumentId, signal?: AbortSignal): Promise<OpenedDocument>;
  saveReadingPosition(id: DocumentId, sentenceId: number): Promise<void>;
  /** Resolves to audio/wav bytes made at 1× speed. */
  getSentenceAudio(request: SentenceAudioRequest, signal?: AbortSignal): Promise<Blob>;
  startExport(documentId: DocumentId, request: StartExportRequest): Promise<AudioExport>;
  /** This document's exports, newest first, so a running or finished export survives closing the reader. */
  listExports(documentId: DocumentId, signal?: AbortSignal): Promise<AudioExport[]>;
  getExport(exportId: string, signal?: AbortSignal): Promise<AudioExport>;
  cancelExport(exportId: string): Promise<void>;
  /** A plain URL for an <a download> link, so a large file streams to disk instead of into memory. */
  exportFileUrl(exportId: string): string;
}

import type { AudioExport, ExportFormat, StartExportRequest } from '../../types/api';
import type { CleanDocument } from '../../types/document';
import { buildPlaybackOrder } from '../../utils/playbackOrder';
import { ApiError } from '../errors';
import { documents } from './library';
import { isScenarioActive } from './scenario';
import { makeMockWav } from './wav';

const EXPORT_DURATION_MS = 20_000;
const QUEUED_MS = 1_000;
const FAIL_AT_SHARE = 0.4;
const MOCK_FILE_SECONDS = 3;
const SUPPORTED_FORMATS: readonly ExportFormat[] = ['mp3', 'm4b'];

interface MockJob { info: AudioExport; startedAt: number; }

const jobs = new Map<string, MockJob>();
let nextJobNumber = 1;
let fileUrl: string | null = null;

/** Same rule as playback, from its single owner. */
const countExportSentences = (document: CleanDocument, readSkipped: boolean): number => buildPlaybackOrder(document, readSkipped).sentenceIds.length;

const notFound = () => new ApiError('ExportNotFound', 3001, 'Export not found.', 404);

/** Module state: kept across reader visits until the page reloads, like the server keeps its jobs. */

/** Progress is computed from the clock, so polling sees it advance over ~20 s. */
const snapshot = (job: MockJob): AudioExport => {
  if (job.info.status === 'cancelled' || job.info.status === 'failed' || job.info.status === 'done') return { ...job.info };
  const elapsed = Date.now() - job.startedAt;
  if (elapsed < QUEUED_MS) return { ...job.info, status: 'queued', estimatedSecondsLeft: Math.round(EXPORT_DURATION_MS / 1000) };
  const share = Math.min(1, (elapsed - QUEUED_MS) / (EXPORT_DURATION_MS - QUEUED_MS));
  const sentencesDone = Math.floor(share * job.info.sentencesTotal);
  if (isScenarioActive('exportFailed') && share >= FAIL_AT_SHARE) job.info = { ...job.info, status: 'failed', sentencesDone, estimatedSecondsLeft: null };
  else if (share >= 1) job.info = { ...job.info, status: 'done', sentencesDone: job.info.sentencesTotal, estimatedSecondsLeft: 0 };
  else job.info = { ...job.info, status: 'running', sentencesDone, estimatedSecondsLeft: Math.ceil((1 - share) * (EXPORT_DURATION_MS - QUEUED_MS) / 1000) };
  return { ...job.info };
};

export const startMockExport = (documentId: string, request: StartExportRequest): AudioExport => {
  const document = documents.get(documentId);
  if (!document) throw new ApiError('DocumentNotFound', 1101, 'Document not found.', 404);
  if (isScenarioActive('exporterMissing')) throw new ApiError('ExporterUnavailable', 3004, "Audio export isn't available (ffmpeg is missing).", 503);
  if (!SUPPORTED_FORMATS.includes(request.format)) throw new ApiError('ExportFormatUnsupported', 3003, "That audio format isn't supported.", 400);
  // Like the server: one active job per document + voice + format; starting it again returns that job.
  const active = [...jobs.values()].map(snapshot).find((info) => info.documentId === documentId && info.voiceId === request.voiceId && info.format === request.format && (info.status === 'queued' || info.status === 'running'));
  if (active) return active;
  const sentencesTotal = countExportSentences(document, request.readSkipped);
  const id = `mock-export-${nextJobNumber++}`;
  const info: AudioExport = {
    id, documentId, voiceId: request.voiceId, format: request.format, speedPercent: 100, engineId: 'mock-engine', range: null, rangeLabel: null,
    title: document.title, status: 'queued', pausedBy: null, stage: 'parts', error: null, sentencesDone: 0, sentencesTotal, partsDone: 0, partsTotal: 1,
    estimatedSecondsLeft: null, fileBytes: null, location: null, waitingForEngine: false,
  };
  const job: MockJob = { startedAt: Date.now(), info };
  jobs.set(id, job);
  return snapshot(job);
};

/** Newest first. Job numbers grow, so creation order is enough. */
export const listMockExports = (documentId: string): AudioExport[] => [...jobs.values()].filter((job) => job.info.documentId === documentId).reverse().map(snapshot);

export const getMockExport = (exportId: string): AudioExport => {
  const job = jobs.get(exportId);
  if (!job) throw notFound();
  return snapshot(job);
};

export const cancelMockExport = (exportId: string): void => {
  const job = jobs.get(exportId);
  if (!job) throw notFound();
  const current = snapshot(job);
  if (current.status === 'queued' || current.status === 'running') job.info = { ...current, status: 'cancelled', estimatedSecondsLeft: null };
};

/** The mock "file" is a short WAV, created once. Fine for testing the download link. */
export const mockExportFileUrl = (): string => {
  fileUrl ??= URL.createObjectURL(makeMockWav(MOCK_FILE_SECONDS));
  return fileUrl;
};

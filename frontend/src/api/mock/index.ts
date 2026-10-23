import type { ImportResult, ListResponse, PageRequest } from '../../types/api';
import type { DocumentSummary, OpenedDocument } from '../../types/document';
import type { LectriceApi, SentenceAudioRequest } from '../contract';
import { ApiError, ClientError } from '../errors';
import { cancelMockExport, getMockExport, listMockExports, mockExportFileUrl, startMockExport } from './exports';
import { delay, jitter } from './latency';
import { addImported, documents, findSentenceText, summaries } from './library';
import { isScenarioActive } from './scenario';
import { MOCK_VOICES } from './voices';
import { makeMockWav, mockSpeechSeconds } from './wav';

const AUDIO_ERROR_SENTENCE_ID = 5;

/** Same message, code and HTTP status the backend registry uses (spec §8). */
const ERRORS = {
  PdfNoText: () => new ApiError('PdfNoText', 1005, 'This PDF has no selectable text.', 422),
  DocumentNotFound: () => new ApiError('DocumentNotFound', 1101, 'Document not found.', 404),
  SentenceNotFound: () => new ApiError('SentenceNotFound', 1102, 'Sentence not found.', 404),
  VoiceUnknown: () => new ApiError('VoiceUnknown', 2001, 'Unknown voice.', 400),
  SpeechEngineUnavailable: () => new ApiError('SpeechEngineUnavailable', 2002, "The speech engine isn't available.", 503),
  SpeechSynthesisFailed: () => new ApiError('SpeechSynthesisFailed', 2003, 'Speech generation failed.', 500),
};

/** Audio already "generated" once comes back fast, like the backend's cache. */
const generatedAudio = new Set<string>();

const respond = async (ms: number, signal?: AbortSignal): Promise<void> => {
  await delay(ms, signal);
  if (isScenarioActive('offline')) throw new ClientError('NetworkError');
};

export const mockApi: LectriceApi = {
  async getHealth(signal) {
    await respond(jitter(80, 60), signal);
    return { status: 'ok', speechEngine: isScenarioActive('engineFailed') ? 'failed' : 'ready' };
  },

  async listVoices(signal) {
    await respond(jitter(120, 80), signal);
    return MOCK_VOICES;
  },

  async listDocuments(page: PageRequest, signal): Promise<ListResponse<DocumentSummary>> {
    await respond(jitter(200, 150), signal);
    const all = isScenarioActive('empty') ? [] : [...summaries].sort((a, b) => b.lastOpenedAt.localeCompare(a.lastOpenedAt));
    return { items: all.slice(page.offset, page.offset + page.limit), total: all.length, limit: page.limit, offset: page.offset };
  },

  async importPdf(file: File, signal): Promise<ImportResult> {
    await respond(jitter(1600, 800), signal);
    if (isScenarioActive('noText')) throw ERRORS.PdfNoText();
    const { summary, alreadyInLibrary } = addImported(file);
    return { document: { ...summary }, alreadyInLibrary };
  },

  async getDocument(id, signal): Promise<OpenedDocument> {
    await respond(jitter(300, 200), signal);
    const document = documents.get(id);
    if (!document) throw ERRORS.DocumentNotFound();
    const summary = summaries.find((item) => item.id === id);
    if (summary) summary.lastOpenedAt = new Date().toISOString();
    return { ...document, lastSentenceId: summary?.lastSentenceId ?? document.lastSentenceId };
  },

  async saveReadingPosition(id, sentenceId) {
    await respond(jitter(60, 60));
    const summary = summaries.find((item) => item.id === id);
    if (!summary) throw ERRORS.DocumentNotFound();
    summary.lastSentenceId = sentenceId;
  },

  async getSentenceAudio({ documentId, sentenceId, voiceId }: SentenceAudioRequest, signal) {
    const key = `${documentId}/${voiceId}/${sentenceId}`;
    await respond(generatedAudio.has(key) ? jitter(20, 30) : jitter(500, 700), signal);
    if (isScenarioActive('engineFailed')) throw ERRORS.SpeechEngineUnavailable();
    if (isScenarioActive('audioError') && sentenceId === AUDIO_ERROR_SENTENCE_ID) throw ERRORS.SpeechSynthesisFailed();
    if (!MOCK_VOICES.voices.some((voice) => voice.id === voiceId)) throw ERRORS.VoiceUnknown();
    const text = findSentenceText(documentId, sentenceId);
    if (text === null) throw ERRORS.SentenceNotFound();
    generatedAudio.add(key);
    return makeMockWav(mockSpeechSeconds(text));
  },
  async startExport(documentId, request) {
    await respond(jitter(150, 100));
    return startMockExport(documentId, request);
  },

  async listExports(documentId, signal) {
    await respond(jitter(80, 60), signal);
    if (!documents.has(documentId)) throw ERRORS.DocumentNotFound();
    return listMockExports(documentId);
  },

  async getExport(exportId, signal) {
    await respond(jitter(80, 60), signal);
    return getMockExport(exportId);
  },

  async cancelExport(exportId) {
    await respond(jitter(80, 60));
    cancelMockExport(exportId);
  },

  exportFileUrl: () => mockExportFileUrl(),
};

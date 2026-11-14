import { getSentenceAudio } from './audio';
import type { LectriceApi } from './contract';
import { getDocument, importPdf, listDocuments, saveReadingPosition } from './documents';
import { cancelExport, exportFileUrl, getExport, listExports, startExport } from './exports';
import { getHealth } from './health';
import { getVoiceSample, listVoices } from './voices';

const httpApi: LectriceApi = { getHealth, listVoices, getVoiceSample, listDocuments, importPdf, getDocument, saveReadingPosition, getSentenceAudio, startExport, listExports, getExport, cancelExport, exportFileUrl };

// Vite replaces VITE_API_MODE at build time, so an http build drops the mock and its sample book.
const loadApi = async (): Promise<LectriceApi> => {
  const mode = import.meta.env.VITE_API_MODE;
  if (mode === 'mock') return (await import('./mock')).mockApi;
  if (mode === 'http') return httpApi;
  throw new Error(`VITE_API_MODE must be "mock" or "http", got "${mode ?? ''}"`);
};

export const api: LectriceApi = await loadApi();

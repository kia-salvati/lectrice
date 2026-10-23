import { API_MODE } from '../config/env';
import type { ApiMode } from '../config/env';
import { getSentenceAudio } from './audio';
import type { LectriceApi } from './contract';
import { getDocument, importPdf, listDocuments, saveReadingPosition } from './documents';
import { cancelExport, exportFileUrl, getExport, listExports, startExport } from './exports';
import { getHealth } from './health';
import { mockApi } from './mock';
import { listVoices } from './voices';

const httpApi: LectriceApi = { getHealth, listVoices, listDocuments, importPdf, getDocument, saveReadingPosition, getSentenceAudio, startExport, listExports, getExport, cancelExport, exportFileUrl };

const API_BY_MODE: Record<ApiMode, LectriceApi> = { http: httpApi, mock: mockApi };

export const api: LectriceApi = API_BY_MODE[API_MODE];

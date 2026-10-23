import { API_BASE_PATH } from '../config/env';
import type { AudioExport, AudioExportList, AudioExportResponse, StartExportRequest } from '../types/api';
import type { DocumentId } from '../types/document';
import { requestJson, requestVoid } from './client';

export const startExport = async (documentId: DocumentId, request: StartExportRequest): Promise<AudioExport> => {
  const response = await requestJson<AudioExportResponse>(`/documents/${encodeURIComponent(documentId)}/exports`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  return response.export;
};

export const listExports = async (documentId: DocumentId, signal?: AbortSignal): Promise<AudioExport[]> => (await requestJson<AudioExportList>(`/documents/${encodeURIComponent(documentId)}/exports`, { signal })).items;

export const getExport = async (exportId: string, signal?: AbortSignal): Promise<AudioExport> => (await requestJson<AudioExportResponse>(`/exports/${encodeURIComponent(exportId)}`, { signal })).export;

export const cancelExport = (exportId: string): Promise<void> => requestVoid(`/exports/${encodeURIComponent(exportId)}`, { method: 'DELETE' });

export const exportFileUrl = (exportId: string): string => `${API_BASE_PATH}/exports/${encodeURIComponent(exportId)}/file`;

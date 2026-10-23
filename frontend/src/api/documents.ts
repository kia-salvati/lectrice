import type { ImportResult, ListResponse, PageRequest } from '../types/api';
import type { DocumentId, DocumentSummary, OpenedDocument } from '../types/document';
import { requestJson, requestVoid } from './client';

export const listDocuments = (page: PageRequest, signal?: AbortSignal): Promise<ListResponse<DocumentSummary>> => {
  const query = new URLSearchParams({ limit: String(page.limit), offset: String(page.offset) });
  return requestJson<ListResponse<DocumentSummary>>(`/documents?${query}`, { signal });
};

export const importPdf = (file: File, signal?: AbortSignal): Promise<ImportResult> => {
  const body = new FormData();
  body.append('file', file);
  return requestJson<ImportResult>('/documents', { method: 'POST', body, signal });
};

export const getDocument = (id: DocumentId, signal?: AbortSignal): Promise<OpenedDocument> => requestJson<OpenedDocument>(`/documents/${encodeURIComponent(id)}`, { signal });

export const saveReadingPosition = (id: DocumentId, sentenceId: number): Promise<void> => requestVoid(`/documents/${encodeURIComponent(id)}/position`, {
  method: 'PUT',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ sentenceId }),
});

import type { ImportResult, ListResponse, PageRequest } from '../types/api';
import type { DocumentId, DocumentSummary, OpenedDocument } from '../types/document';
import { serializeFormData } from '../utils/multipart';
import { requestJson, requestVoid } from './client';

export const listDocuments = (page: PageRequest, signal?: AbortSignal): Promise<ListResponse<DocumentSummary>> => {
  const query = new URLSearchParams({ limit: String(page.limit), offset: String(page.offset) });
  return requestJson<ListResponse<DocumentSummary>>(`/documents?${query}`, { signal });
};

export const importPdf = async (file: File, signal?: AbortSignal): Promise<ImportResult> => {
  const form = new FormData();
  form.append('file', file);
  const { body, contentType } = await serializeFormData(form);
  return requestJson<ImportResult>('/documents', { method: 'POST', headers: { 'Content-Type': contentType }, body, signal });
};

export const getDocument = (id: DocumentId, signal?: AbortSignal): Promise<OpenedDocument> => requestJson<OpenedDocument>(`/documents/${encodeURIComponent(id)}`, { signal });

export const saveReadingPosition = (id: DocumentId, sentenceId: number): Promise<void> => requestVoid(`/documents/${encodeURIComponent(id)}/position`, {
  method: 'PUT',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ sentenceId }),
});

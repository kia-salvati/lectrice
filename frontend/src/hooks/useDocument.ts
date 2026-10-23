import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import { isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import type { DocumentId, OpenedDocument } from '../types/document';

export interface DocumentLoad {
  document: OpenedDocument | null;
  status: 'loading' | 'ready' | 'failed';
  error: AppError | null;
  retry: () => void;
}

/** Loads one document with its saved position. The caller remounts it (key) when the id changes. */
export const useDocument = (documentId: DocumentId): DocumentLoad => {
  const [document, setDocument] = useState<OpenedDocument | null>(null);
  const [status, setStatus] = useState<DocumentLoad['status']>('loading');
  const [error, setError] = useState<AppError | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    api.getDocument(documentId, controller.signal).then((opened) => {
      setDocument(opened);
      setStatus('ready');
    }).catch((cause: unknown) => {
      if (isAbortError(cause)) return;
      const appError = toAppError(cause);
      console.error('Opening document failed', appError);
      setError(appError);
      setStatus('failed');
    });
    return () => controller.abort();
  }, [documentId, attempt]);

  const retry = useCallback(() => {
    setStatus('loading');
    setError(null);
    setAttempt((count) => count + 1);
  }, []);

  return { document, status, error, retry };
};

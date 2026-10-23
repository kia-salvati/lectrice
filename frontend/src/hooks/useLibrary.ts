import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api';
import { ClientError, isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import { MAX_UPLOAD_BYTES, RECENT_PAGE_SIZE } from '../config/constants';
import type { DocumentId, DocumentSummary } from '../types/document';
import { PDF_MAGIC, checkPdfFile } from '../utils/pdfFile';

export type ImportState =
  | { phase: 'idle' }
  | { phase: 'importing'; fileName: string }
  | { phase: 'failed'; fileName: string; error: AppError };

export interface UseLibraryArgs { onDocumentReady: (documentId: DocumentId) => void; }

export interface Library {
  recent: DocumentSummary[];
  total: number;
  recentStatus: 'loading' | 'ready' | 'failed';
  recentError: AppError | null;
  /** When the list was fetched; "opened 2 hours ago" is measured from here. */
  recentLoadedAt: Date;
  isLoadingMore: boolean;
  reloadRecent: () => void;
  loadMore: () => void;
  importState: ImportState;
  importFile: (file: File) => void;
  resetImport: () => void;
}

/** Recent documents (paged) and the PDF import flow. */
export const useLibrary = ({ onDocumentReady }: UseLibraryArgs): Library => {
  const [recent, setRecent] = useState<DocumentSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [recentStatus, setRecentStatus] = useState<Library['recentStatus']>('loading');
  const [recentError, setRecentError] = useState<AppError | null>(null);
  const [recentLoadedAt, setRecentLoadedAt] = useState(() => new Date());
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const [importState, setImportState] = useState<ImportState>({ phase: 'idle' });

  const onDocumentReadyRef = useRef(onDocumentReady);
  useEffect(() => {
    onDocumentReadyRef.current = onDocumentReady;
  });

  // Requests started from actions (load more, import) are aborted when the page goes away.
  const actionsRef = useRef<AbortController | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    actionsRef.current = controller;
    return () => controller.abort();
  }, []);

  const failRecent = useCallback((cause: unknown) => {
    if (isAbortError(cause)) return;
    const error = toAppError(cause);
    console.error('Recent documents failed', error);
    setRecentError(error);
    setRecentStatus('failed');
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    api.listDocuments({ limit: RECENT_PAGE_SIZE, offset: 0 }, controller.signal).then((page) => {
      setRecent(page.items);
      setTotal(page.total);
      setRecentLoadedAt(new Date());
      setRecentError(null);
      setRecentStatus('ready');
    }).catch(failRecent);
    return () => controller.abort();
  }, [reloadToken, failRecent]);

  const reloadRecent = useCallback(() => {
    setRecentStatus('loading');
    setReloadToken((token) => token + 1);
  }, []);

  const loadMore = useCallback(() => {
    if (isLoadingMore) return;
    setIsLoadingMore(true);
    api.listDocuments({ limit: RECENT_PAGE_SIZE, offset: recent.length }, actionsRef.current?.signal).then((page) => {
      setRecent((current) => [...current, ...page.items]);
      setTotal(page.total);
    }).catch(failRecent).finally(() => setIsLoadingMore(false));
  }, [isLoadingMore, recent.length, failRecent]);

  const importingRef = useRef(false);
  const importFile = useCallback(async (file: File) => {
    if (importingRef.current) return;
    importingRef.current = true;
    const fail = (error: AppError) => {
      importingRef.current = false;
      setImportState({ phase: 'failed', fileName: file.name, error });
    };
    const quickCheck = checkPdfFile(file, MAX_UPLOAD_BYTES);
    if (quickCheck) {
      fail(new ClientError(quickCheck));
      return;
    }
    if ((await file.slice(0, PDF_MAGIC.length).text()) !== PDF_MAGIC) {
      fail(new ClientError('FileNotPdf'));
      return;
    }
    setImportState({ phase: 'importing', fileName: file.name });
    try {
      const result = await api.importPdf(file, actionsRef.current?.signal);
      importingRef.current = false;
      onDocumentReadyRef.current(result.document.id);
    } catch (cause) {
      if (isAbortError(cause)) return;
      console.error('Import failed', cause);
      fail(toAppError(cause));
    }
  }, []);

  const resetImport = useCallback(() => setImportState({ phase: 'idle' }), []);

  return { recent, total, recentStatus, recentError, recentLoadedAt, isLoadingMore, reloadRecent, loadMore, importState, importFile, resetImport };
};

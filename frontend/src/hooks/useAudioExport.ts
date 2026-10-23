import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api';
import { hasErrorName, isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import { EXPORT_FORMATS, EXPORT_POLL_INTERVAL_MS } from '../config/constants';
import type { AudioExport, ExportFormat, ExportStatus } from '../types/api';
import type { DocumentId } from '../types/document';

const IN_PROGRESS: Record<ExportStatus, boolean> = { queued: true, running: true, done: false, failed: false, cancelled: false };

export interface AudioExportState {
  /** The latest known job, or null before the first export in this session. */
  job: AudioExport | null;
  isStarting: boolean;
  /** A request failed (list, start, poll or cancel). The job's own `failed` status is separate; engine failures go to onEngineUnavailable instead. */
  error: AppError | null;
  /** Set only when the job is done. */
  fileUrl: string | null;
  /** The format the next export will use (what will be submitted, so it lives here, not in the view). */
  format: ExportFormat;
  setFormat: (format: ExportFormat) => void;
  start: (voiceId: string, readSkipped: boolean) => void;
  cancel: () => void;
}

/** The document's latest whole-book export. Loaded on open (GET …/exports, newest first), so a running or finished export survives closing the reader. */
export const useAudioExport = (documentId: DocumentId, onEngineUnavailable: () => void): AudioExportState => {
  const [job, setJob] = useState<AudioExport | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [format, setFormat] = useState<ExportFormat>(EXPORT_FORMATS[0]);

  const engineUnavailableRef = useRef(onEngineUnavailable);
  useEffect(() => {
    engineUnavailableRef.current = onEngineUnavailable;
  });

  // One failure path for every request. An engine-down answer is the engine's news, shown once by its banner
  // and the disabled start button, so it never becomes a second message in the panel.
  const fail = useCallback((cause: unknown) => {
    if (isAbortError(cause)) return;
    if (hasErrorName(cause, 'SpeechEngineUnavailable')) {
      engineUnavailableRef.current();
      return;
    }
    setError(toAppError(cause));
  }, []);

  // Pick up the newest existing export, unless one was started in the meantime.
  useEffect(() => {
    const controller = new AbortController();
    api.listExports(documentId, controller.signal).then((items) => {
      const latest = items[0];
      if (latest) setJob((current) => current ?? latest);
    }).catch(fail);
    return () => controller.abort();
  }, [documentId, fail]);

  const start = useCallback((voiceId: string, readSkipped: boolean) => {
    setIsStarting(true);
    setError(null);
    api.startExport(documentId, { voiceId, format, readSkipped })
      .then(setJob)
      .catch(fail)
      .finally(() => setIsStarting(false));
  }, [documentId, format, fail]);

  const jobId = job?.id ?? null;
  const inProgress = job !== null && IN_PROGRESS[job.status];

  useEffect(() => {
    if (jobId === null || !inProgress) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      api.getExport(jobId, controller.signal).then(setJob).catch(fail);
    }, EXPORT_POLL_INTERVAL_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [jobId, inProgress, job, fail]);

  const cancel = useCallback(() => {
    if (jobId === null) return;
    api.cancelExport(jobId)
      .then(() => api.getExport(jobId))
      .then(setJob)
      .catch(fail);
  }, [jobId, fail]);

  const fileUrl = job?.status === 'done' ? api.exportFileUrl(job.id) : null;

  return { job, isStarting, error, fileUrl, format, setFormat, start, cancel };
};

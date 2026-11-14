import type { AudioExport, ExportStatus } from '../../types/api';
import { isStatusInProgress } from '../../utils/exportStatus.ts';

export type ExportToastKind = 'ready' | 'stopped';

/** A finished export is "ready"; one a full disk or an error stopped is "stopped". Pauses by the user or the app, and cancels, say nothing. */
export const toastKindOf = (job: AudioExport): ExportToastKind | null => {
  if (job.status === 'done') return 'ready';
  if (job.status === 'failed' || (job.status === 'paused' && job.pausedBy === 'storageFull')) return 'stopped';
  return null;
};

/**
 * The exports that ended since the last look: they were in progress then, or didn't exist yet. The first look
 * (`previous` null) only learns what is there, so old exports never pop up at start.
 */
export const newlyEnded = (previous: ReadonlyMap<string, ExportStatus> | null, current: readonly AudioExport[]): AudioExport[] => {
  if (previous === null) return [];
  return current.filter((job) => {
    const before = previous.get(job.id);
    return toastKindOf(job) !== null && (before === undefined || isStatusInProgress(before));
  });
};

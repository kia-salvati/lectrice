import type { AudioExport } from '../../types/api';

export type DesktopExportPhase = 'unavailable' | 'idle' | 'starting' | 'queued' | 'waitingForModel' | 'running' | 'storageFull' | 'failed' | 'done' | 'moving' | 'saved' | 'saveFailed' | 'fileGone' | 'cancelled';

/** What happened after the file was ready: a move under way, a failed move, a reveal that found no file, or a file manager that didn't start. */
export type ExportSaveState = { kind: 'idle' } | { kind: 'moving' } | { kind: 'failed'; storageFull: boolean } | { kind: 'fileGone' } | { kind: 'revealFailed' };

/** A file manager that didn't start is told by the toast only; the panel keeps showing the file. */
const SAVE_PHASE: Record<ExportSaveState['kind'], DesktopExportPhase | null> = { idle: null, revealFailed: null, moving: 'moving', failed: 'saveFailed', fileGone: 'fileGone' };

/**
 * One phase for the desktop export panel. No exporter wins over everything; then a start in flight; then the job.
 * "Saved" is the job's own location (the core marks the move), so it survives closing the reader.
 * A pause for any reason but a full disk shows as stopped, as the web panel does.
 */
export const desktopExportPhase = (job: AudioExport | null, isStarting: boolean, save: ExportSaveState, exportAvailable: boolean): DesktopExportPhase => {
  if (!exportAvailable) return 'unavailable';
  if (isStarting) return 'starting';
  if (job === null) return 'idle';
  if (job.status === 'done') {
    const savePhase = SAVE_PHASE[save.kind];
    if (savePhase !== null) return savePhase;
    return job.location?.kind === 'shared' ? 'saved' : 'done';
  }
  if (job.waitingForEngine) return 'waitingForModel';
  if (job.status === 'paused') return job.pausedBy === 'storageFull' ? 'storageFull' : 'failed';
  return job.status;
};

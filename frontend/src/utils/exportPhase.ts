import type { AudioExport } from '../types/api';

export type ExportPhase = 'idle' | 'starting' | 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

/** One phase for the export panel: a start request in flight wins, then the job's own status. */
export const exportPhase = (job: AudioExport | null, isStarting: boolean): ExportPhase => {
  if (isStarting) return 'starting';
  if (job === null) return 'idle';
  return job.status;
};

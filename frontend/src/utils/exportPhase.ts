import type { AudioExport } from '../types/api';

export type ExportPhase = 'idle' | 'starting' | 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

/** One phase for the export panel: a start request in flight wins, then the job's own status. A paused export (on the
 * desktop: the disk ran out of space) shows as stopped; "Try again" continues it from its finished parts. */
export const exportPhase = (job: AudioExport | null, isStarting: boolean): ExportPhase => {
  if (isStarting) return 'starting';
  if (job === null) return 'idle';
  if (job.status === 'paused') return 'failed';
  return job.status;
};

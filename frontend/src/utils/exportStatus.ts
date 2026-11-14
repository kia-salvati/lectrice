import type { AudioExport, ExportStatus } from '../types/api';

const IN_PROGRESS: Record<ExportStatus, boolean> = { queued: true, running: true, paused: false, done: false, failed: false, cancelled: false };

export const isStatusInProgress = (status: ExportStatus): boolean => IN_PROGRESS[status];

export const isInProgress = (job: AudioExport): boolean => IN_PROGRESS[job.status];

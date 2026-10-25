import type { AudioExport } from '../../types/api';
import type { ExportSaveState } from './exportPhase';

/**
 * What happened to a finished file in this session; `saved` until the job itself says where the file went.
 * A failed reveal keeps the place a save in this session moved the file to.
 */
export type SaveEntry =
  | Exclude<ExportSaveState, { kind: 'idle' } | { kind: 'revealFailed' }>
  | { kind: 'saved'; path: string }
  | { kind: 'revealFailed'; savedPath: string | null };

const IDLE: ExportSaveState = { kind: 'idle' };
const REVEAL_FAILED: ExportSaveState = { kind: 'revealFailed' };

const sessionSavedPath = (entry: SaveEntry | undefined): string | null => {
  if (entry?.kind === 'saved') return entry.path;
  return entry?.kind === 'revealFailed' ? entry.savedPath : null;
};

export const saveStateOfEntry = (entry: SaveEntry | undefined): ExportSaveState => {
  if (entry === undefined || entry.kind === 'saved') return IDLE;
  return entry.kind === 'revealFailed' ? REVEAL_FAILED : entry;
};

export const savedPathOfEntry = (entry: SaveEntry | undefined, job: AudioExport): string | null =>
  sessionSavedPath(entry) ?? (job.location?.kind === 'shared' ? job.location.uri : null);

export const afterRevealFailed = (entry: SaveEntry | undefined): SaveEntry => ({ kind: 'revealFailed', savedPath: sessionSavedPath(entry) });

/** A reveal that worked clears its earlier failure; anything else stays as it was. */
export const afterRevealed = (entry: SaveEntry | undefined): SaveEntry | undefined => {
  if (entry?.kind !== 'revealFailed') return entry;
  return entry.savedPath === null ? undefined : { kind: 'saved', path: entry.savedPath };
};

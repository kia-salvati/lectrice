import { useCallback, useState } from 'react';
import type { AudioExport } from '../../types/api';
import { hasDesktopErrorName } from '../commands/errors';
import type { DesktopCommands } from '../commands/types';
import type { ExportSaveState } from '../utils/exportPhase';
import { afterRevealed, afterRevealFailed, savedPathOfEntry, saveStateOfEntry } from '../utils/exportSaves';
import type { SaveEntry } from '../utils/exportSaves';

export interface ExportSaves {
  /** For the export panel's phase and the toast: what the job alone doesn't say yet. */
  saveStateOf: (exportId: string) => ExportSaveState;
  /** Where the file is outside Lectrice, from this session's "Save as…" or from the job. */
  savedPathOf: (job: AudioExport) => string | null;
  saveAs: (exportId: string) => void;
  reveal: (exportId: string) => void;
}

/** "Save as…" and "Show in folder" for any export, shared by the reader's panel and the toast so both show the same. */
export const useExportSaves = (commands: DesktopCommands): ExportSaves => {
  const [entries, setEntries] = useState<ReadonlyMap<string, SaveEntry>>(new Map());

  const update = useCallback((exportId: string, next: (entry: SaveEntry | undefined) => SaveEntry | undefined) => {
    setEntries((current) => {
      const before = current.get(exportId);
      const after = next(before);
      if (after === before) return current;
      const updated = new Map(current);
      if (after === undefined) updated.delete(exportId);
      else updated.set(exportId, after);
      return updated;
    });
  }, []);

  const saveAs = useCallback((exportId: string) => {
    update(exportId, () => ({ kind: 'moving' }));
    commands.exportSaveAs(exportId).then((result) => {
      update(exportId, () => (result.outcome === 'saved' ? { kind: 'saved', path: result.path } : undefined));
    }).catch((cause: unknown) => {
      console.error('Save as failed', cause);
      update(exportId, () => ({ kind: 'failed', storageFull: hasDesktopErrorName(cause, 'StorageFull') }));
    });
  }, [commands, update]);

  const reveal = useCallback((exportId: string) => {
    commands.exportReveal(exportId).then(() => update(exportId, afterRevealed)).catch((cause: unknown) => {
      if (hasDesktopErrorName(cause, 'ExportNotFound')) {
        update(exportId, () => ({ kind: 'fileGone' }));
        return;
      }
      console.error('Show in folder failed', cause);
      update(exportId, afterRevealFailed);
    });
  }, [commands, update]);

  const saveStateOf = (exportId: string): ExportSaveState => saveStateOfEntry(entries.get(exportId));
  const savedPathOf = (job: AudioExport): string | null => savedPathOfEntry(entries.get(job.id), job);

  return { saveStateOf, savedPathOf, saveAs, reveal };
};

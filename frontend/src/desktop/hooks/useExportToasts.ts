import { useCallback, useEffect, useRef, useState } from 'react';
import type { AudioExport, ExportStatus } from '../../types/api';
import type { DesktopCommands } from '../commands/types';
import { isInProgress, newlyEnded } from '../utils/exportToasts';

/** How often every book's exports are looked at while one is in progress. Nothing is polled otherwise. */
const POLL_INTERVAL_MS = 5000;

export interface ExportToasts {
  /** The oldest export that ended and wasn't dismissed yet, as it was when it ended. */
  current: AudioExport | null;
  dismiss: (exportId: string) => void;
  /** Look again now, and keep looking while something is in progress: an export was started or the engine changed. */
  wake: () => void;
}

/** The "ready" toast's source: exports of all books that finish or stop while the app runs, one toast each. */
export const useExportToasts = (commands: DesktopCommands): ExportToasts => {
  const [ended, setEnded] = useState<readonly AudioExport[]>([]);
  const [wakeCount, setWakeCount] = useState(0);
  const known = useRef<ReadonlyMap<string, ExportStatus> | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let live = true;
    const look = () => {
      commands.exportList().then((jobs) => {
        if (!live) return;
        const fresh = newlyEnded(known.current, jobs);
        known.current = new Map(jobs.map((job) => [job.id, job.status]));
        if (fresh.length > 0) setEnded((current) => [...current, ...fresh.filter((job) => !current.some((shown) => shown.id === job.id))]);
        if (jobs.some(isInProgress)) timer = setTimeout(look, POLL_INTERVAL_MS);
      }).catch((cause: unknown) => {
        // The engine is being swapped, for example; the next wake looks again.
        console.error('Export list failed', cause);
      });
    };
    look();
    return () => {
      live = false;
      if (timer !== null) clearTimeout(timer);
    };
  }, [commands, wakeCount]);

  const dismiss = useCallback((exportId: string) => setEnded((current) => current.filter((job) => job.id !== exportId)), []);
  const wake = useCallback(() => setWakeCount((count) => count + 1), []);

  return { current: ended[0] ?? null, dismiss, wake };
};

import type { AudioExport } from '../types/api';
import { ExportToast } from './components/ExportToast/ExportToast';
import type { ExportSaves } from './hooks/useExportSaves';
import { formatDuration, m4bSeconds } from './utils/format';

interface DesktopExportToastProps {
  job: AudioExport;
  saves: ExportSaves;
  /** The reader's chapters rail is open (the toast moves above the pill). */
  railOpen: boolean;
  onDismiss: () => void;
  onOpenBook: () => void;
}

/** The toast for one export that ended: ready (then moving, saved, a failed save or reveal), stopped, or its file gone. */
export const DesktopExportToast = ({ job, saves, railOpen, onDismiss, onOpenBook }: DesktopExportToastProps) => {
  const base = { bookTitle: job.title, railOpen, onDismiss };
  if (job.status !== 'done') {
    const reasonText = job.pausedBy === 'storageFull' ? `${job.title}: the disk is full.` : `${job.title}: the export couldn't continue.`;
    return <ExportToast kind="stopped" reasonText={reasonText} onOpenBook={onOpenBook} {...base} />;
  }
  const durationText = job.fileBytes === null ? '' : formatDuration(m4bSeconds(job.fileBytes));
  const onSaveAs = () => saves.saveAs(job.id);
  const onReveal = () => saves.reveal(job.id);
  const save = saves.saveStateOf(job.id);
  switch (save.kind) {
    case 'moving':
      return <ExportToast kind="moving" durationText={durationText} {...base} />;
    case 'failed':
      return <ExportToast kind="saveFailed" storageFull={save.storageFull} onSaveAs={onSaveAs} onReveal={onReveal} {...base} />;
    case 'fileGone':
      return <ExportToast kind="fileGone" onOpenBook={onOpenBook} {...base} />;
    case 'revealFailed':
      return <ExportToast kind="revealFailed" onReveal={onReveal} {...base} />;
    case 'idle': {
      const savedPath = saves.savedPathOf(job);
      if (savedPath !== null) return <ExportToast kind="saved" durationText={durationText} path={savedPath} onReveal={onReveal} {...base} />;
      return <ExportToast kind="ready" durationText={durationText} onSaveAs={onSaveAs} onReveal={onReveal} {...base} />;
    }
  }
};

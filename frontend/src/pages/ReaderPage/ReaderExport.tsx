import { ExportPanel } from '../../components/ExportPanel/ExportPanel';
import { EXPORT_FORMAT_EXTENSION } from '../../config/constants';
import type { AudioExportState } from '../../hooks/useAudioExport';
import { exportPhase } from '../../utils/exportPhase';
import { safeFileName } from '../../utils/fileName';
import { formatTimeLeft } from '../../utils/time';
import type { ReaderShell } from './readerShell';

interface ReaderExportProps {
  documentTitle: string;
  exportState: AudioExportState;
  /** The next export's voice. */
  voiceName: string;
  /** The finished (or latest) job's voice. */
  fileVoiceName: string;
  readSkipped: boolean;
  canStart: boolean;
  onStart: () => void;
  bookSeconds: number;
  shell?: ReaderShell;
  /** Around the shell's panel, so the reader can scroll it into view. */
  shellPanelRef: React.Ref<HTMLDivElement>;
}

/** The popover's export section: the shell's own panel when there is a shell, else the web app's. */
export const ReaderExport = ({ documentTitle, exportState, voiceName, fileVoiceName, readSkipped, canStart, onStart, bookSeconds, shell, shellPanelRef }: ReaderExportProps) => {
  if (shell) return <div ref={shellPanelRef}>{shell.renderExportPanel({ exportState, voiceName, readSkipped, fileVoiceName, canStart, onStart, bookSeconds })}</div>;
  const job = exportState.job;
  const fileFormat = job?.format ?? exportState.format;
  return (
    <ExportPanel
      phase={exportPhase(job, exportState.isStarting)}
      format={exportState.format}
      onFormatChange={exportState.setFormat}
      fileFormat={fileFormat}
      fileVoiceName={fileVoiceName}
      sentencesDone={job?.sentencesDone ?? 0}
      sentencesTotal={job?.sentencesTotal ?? 0}
      timeLeftText={job?.estimatedSecondsLeft == null ? null : formatTimeLeft(job.estimatedSecondsLeft)}
      voiceName={voiceName}
      readSkipped={readSkipped}
      fileUrl={exportState.fileUrl}
      downloadName={safeFileName(`${documentTitle} (${fileVoiceName}).${EXPORT_FORMAT_EXTENSION[fileFormat]}`)}
      errorMessage={exportState.error?.message ?? null}
      canStart={canStart}
      onStart={onStart}
      onCancel={exportState.cancel}
    />
  );
};

import { useEffect } from 'react';
import type { ExportPanelContext } from '../pages/ReaderPage/readerShell';
import type { AudioExport } from '../types/api';
import type { ModelStatus } from './commands/types';
import { DesktopExportPanel } from './components/ExportPanel/DesktopExportPanel';
import type { ExportAvailabilityState } from './hooks/useExportAvailability';
import type { ExportSaves } from './hooks/useExportSaves';
import { desktopExportPhase } from './utils/exportPhase';
import type { ExportSaveState } from './utils/exportPhase';
import { isInProgress } from './utils/exportToasts';
import { formatApproxTimeLeft, formatDuration, m4bSeconds } from './utils/format';

interface DesktopReaderExportProps {
  context: ExportPanelContext;
  status: ModelStatus;
  saves: ExportSaves;
  exporter: ExportAvailabilityState;
  /** The book's export is under way: the toast should watch for its end. */
  onExportActive: () => void;
}

const NOTHING_SAVED: ExportSaveState = { kind: 'idle' };

/** The job as the panel shows it: a file saved in this session counts as saved before the job is read again. */
const withSavedPath = (job: AudioExport, savedPath: string | null): AudioExport =>
  savedPath === null || job.location?.kind === 'shared' ? job : { ...job, location: { kind: 'shared', uri: savedPath } };

/** The reader's export section on the desktop: the web hook's job, plus Save as…, Show in folder and the ffmpeg check. */
export const DesktopReaderExport = ({ context, status, saves, exporter, onExportActive }: DesktopReaderExportProps) => {
  const { exportState } = context;
  const job = exportState.job;
  const savedPath = job === null ? null : saves.savedPathOf(job);
  const save = job === null ? NOTHING_SAVED : saves.saveStateOf(job.id);
  const phase = desktopExportPhase(job === null ? null : withSavedPath(job, savedPath), exportState.isStarting, save, exporter.availability.available);

  const isActive = job !== null && isInProgress(job);
  useEffect(() => {
    if (isActive) onExportActive();
  }, [isActive, onExportActive]);

  // A waiting export names the model it was made with; everything else the model in use.
  const jobBundle = job === null ? undefined : status.bundles.find((bundle) => bundle.usedByExports.some((use) => use.exportId === job.id));
  const activeBundle = status.bundles.find((bundle) => bundle.active);
  const modelName = (phase === 'waitingForModel' ? jobBundle : activeBundle)?.displayName ?? '';
  const fileBytes = job?.fileBytes ?? null;
  const durationText = formatDuration(fileBytes === null ? context.bookSeconds : m4bSeconds(fileBytes));

  return (
    <DesktopExportPanel
      phase={phase}
      unavailableReason={exporter.availability.available ? null : exporter.availability.reason}
      isCheckingExporter={exporter.isChecking}
      onCheckAgain={exporter.checkAgain}
      voiceName={context.voiceName}
      modelName={modelName}
      readSkipped={context.readSkipped}
      durationText={durationText}
      fileVoiceName={context.fileVoiceName}
      fileBytes={fileBytes}
      sentencesDone={job?.sentencesDone ?? 0}
      sentencesTotal={job?.sentencesTotal ?? 0}
      timeLeftText={job?.estimatedSecondsLeft == null ? null : formatApproxTimeLeft(job.estimatedSecondsLeft)}
      savedPath={savedPath}
      saveFailedStorageFull={save.kind === 'failed' && save.storageFull}
      canStart={context.canStart}
      onStart={context.onStart}
      onCancel={exportState.cancel}
      onSaveAs={() => job && saves.saveAs(job.id)}
      onReveal={() => job && saves.reveal(job.id)}
    />
  );
};

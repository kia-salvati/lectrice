import { useState } from 'react';
import { CheckIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { ExportUnavailableReason } from '../../commands/types';
import { FFMPEG_INSTALL_COMMANDS } from '../../config';
import desktop from '../../styles/controls.module.css';
import type { DesktopExportPhase } from '../../utils/exportPhase';
import { folderNameOf, formatCount, formatMb, percentOf } from '../../utils/format';
import { CommandRow } from '../CommandRow/CommandRow';
import { ConfirmDialog } from '../ModalDialog/ModalDialog';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { PathText } from '../PathText/PathText';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import styles from './DesktopExportPanel.module.css';

export interface DesktopExportPanelProps {
  phase: DesktopExportPhase;
  /** Why the exporter is missing, in the `unavailable` phase. */
  unavailableReason: ExportUnavailableReason | null;
  /** "Check again" is running. */
  isCheckingExporter: boolean;
  onCheckAgain: () => void;
  /** The next export: its voice, model and what it does with code and tables. */
  voiceName: string;
  modelName: string;
  readSkipped: boolean;
  /** Length of the audio: "8 h 10 min". */
  durationText: string;
  /** The finished (or latest) job. */
  fileVoiceName: string;
  fileBytes: number | null;
  sentencesDone: number;
  sentencesTotal: number;
  timeLeftText: string | null;
  /** Where the file went after "Save as…". */
  savedPath: string | null;
  /** The failed move was refused for lack of space on the target drive. */
  saveFailedStorageFull: boolean;
  /** The speech engine is ready. */
  canStart: boolean;
  /** Start, Continue, Start again and Export again. */
  onStart: () => void;
  onCancel: () => void;
  onSaveAs: () => void;
  onReveal: () => void;
}

const SAVE_AS_HELP = 'Moves the file out of Lectrice to a folder you pick, such as Music.';

const CancelButton = ({ onAsk }: { onAsk: () => void }) => <button type="button" className={desktop.textButton} onClick={onAsk}>Cancel export</button>;
const ExportAgainButton = ({ onStart }: Pick<DesktopExportPanelProps, 'onStart'>) => <button type="button" className={desktop.textButton} onClick={onStart}>Export again</button>;
const Ok = ({ children }: { children: React.ReactNode }) => <span className={desktop.ok}><CheckIcon /><span>{children}</span></span>;

const IdleControls = (props: DesktopExportPanelProps) => (
  <>
    <p>One M4B file with chapter marks. Plays in any audiobook app.</p>
    <p className={styles.help}>{props.voiceName} voice · {props.modelName} · code and tables: {props.readSkipped ? 'read aloud' : 'cue only'} · about {props.durationText} of audio</p>
    <div className={desktop.actions}>
      <button type="button" className={controls.button} disabled={!props.canStart} onClick={props.onStart}>{props.canStart ? 'Export whole book (M4B)' : 'Needs the speech engine'}</button>
    </div>
  </>
);

const FfmpegHelp = ({ heading, text, isChecking, onCheckAgain }: { heading: string; text: string; isChecking: boolean; onCheckAgain: () => void }) => (
  <NoticeBox kind="info">
    <p><b>{heading}</b></p>
    <p>{text}</p>
    {FFMPEG_INSTALL_COMMANDS.map((item) => <CommandRow key={item.label} label={item.label} command={item.command} />)}
    <p className={desktop.muted}>Other Linux: install “ffmpeg” with your package manager.</p>
    <div className={desktop.actions}><button type="button" className={controls.button} disabled={isChecking} onClick={onCheckAgain}>{isChecking ? 'Checking…' : 'Check again'}</button></div>
  </NoticeBox>
);

const UNAVAILABLE: Record<ExportUnavailableReason, (props: DesktopExportPanelProps) => React.ReactNode> = {
  ffmpegMissing: (props) => <FfmpegHelp heading="Exporting needs FFmpeg" text="Lectrice uses the free FFmpeg program to make the audiobook file. Install it, then check again." isChecking={props.isCheckingExporter} onCheckAgain={props.onCheckAgain} />,
  ffmpegCannotMakeM4b: (props) => <FfmpegHelp heading="Your FFmpeg can't make M4B files" text="It's missing the AAC encoder or the M4B format. Install your distribution's standard ffmpeg package, then check again." isChecking={props.isCheckingExporter} onCheckAgain={props.onCheckAgain} />,
  bundledFfmpegMissing: () => <NoticeBox kind="error"><p>Exporting isn't available: part of Lectrice is missing. Reinstall Lectrice to fix it.</p></NoticeBox>,
};

const PHASE_VIEW: Record<DesktopExportPhase, (props: DesktopExportPanelProps, askCancel: () => void) => React.ReactNode> = {
  unavailable: (props) => (props.unavailableReason === null ? null : UNAVAILABLE[props.unavailableReason](props)),
  idle: (props) => <IdleControls {...props} />,
  cancelled: (props) => <><p>Export cancelled.</p><IdleControls {...props} /></>,
  starting: () => <div className={desktop.actions}><button type="button" className={controls.button} disabled>Starting export…</button></div>,
  queued: (_props, askCancel) => <><p>Waiting to start…</p><ProgressBar percent={null} label="Export progress" /><div className={desktop.actions}><CancelButton onAsk={askCancel} /></div></>,
  waitingForModel: (props, askCancel) => <><p>Waiting: this export uses the {props.modelName} model. It continues when {props.modelName} is in use again (Settings → Voice model).</p><div className={desktop.actions}><CancelButton onAsk={askCancel} /></div></>,
  running: (props, askCancel) => {
    const percent = percentOf(props.sentencesDone, props.sentencesTotal);
    const countText = `${formatCount(props.sentencesDone)} of ${formatCount(props.sentencesTotal)} sentences`;
    return (
      <>
        <p className={styles.title}>Exporting · {percent} %</p>
        <ProgressBar percent={percent} valueText={countText} label="Export progress" />
        <p>{countText}{props.timeLeftText ? ` · ${props.timeLeftText}` : ''}</p>
        <p className={styles.help}>Listening comes first: the export slows down while you listen.</p>
        <p>You can keep reading or close this. If you quit Lectrice, the export continues the next time you open it.</p>
        <div className={desktop.actions}><CancelButton onAsk={askCancel} /></div>
      </>
    );
  },
  storageFull: (props, askCancel) => (
    <>
      <NoticeBox kind="error"><p>The export stopped: the disk is full. The finished parts are kept. Free up space, then continue.</p></NoticeBox>
      <div className={desktop.actions}><button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onStart}>Continue</button><CancelButton onAsk={askCancel} /></div>
    </>
  ),
  failed: (props) => (
    <>
      <NoticeBox kind="error"><p>The export stopped and can't continue. Start it again.</p></NoticeBox>
      <div className={desktop.actions}><button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onStart}>Start again</button></div>
    </>
  ),
  done: (props) => (
    <>
      <Ok>Audiobook ready</Ok>
      <p>{props.durationText}{props.fileBytes === null ? '' : ` · ${formatMb(props.fileBytes)}`} · M4B · {props.fileVoiceName}</p>
      <div className={desktop.actions}>
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onSaveAs}>Save as…</button>
        <button type="button" className={controls.button} onClick={props.onReveal}>Show in folder</button>
        <ExportAgainButton onStart={props.onStart} />
      </div>
      <p className={styles.help}>{SAVE_AS_HELP}</p>
    </>
  ),
  moving: () => <><p>Moving…</p><ProgressBar percent={null} label="Moving the file" /></>,
  saved: (props) => (
    <>
      <Ok>Saved to {props.savedPath === null ? '' : folderNameOf(props.savedPath)}</Ok>
      {props.savedPath !== null && <PathText path={props.savedPath} />}
      <div className={desktop.actions}>
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onReveal}>Show in folder</button>
        <ExportAgainButton onStart={props.onStart} />
      </div>
    </>
  ),
  saveFailed: (props) => (
    <>
      <NoticeBox kind="error"><p>{props.saveFailedStorageFull ? "There isn't enough space on that drive. The file is still in Lectrice." : "Couldn't save there. The file is still in Lectrice."}</p></NoticeBox>
      <div className={desktop.actions}><button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onSaveAs}>Save as…</button></div>
    </>
  ),
  fileGone: (props) => (
    <>
      <NoticeBox kind="error"><p>The file isn't there any more. It may have been moved or deleted.</p></NoticeBox>
      <div className={desktop.actions}><ExportAgainButton onStart={props.onStart} /></div>
    </>
  ),
};

/** UX3: the pill popover's export section on desktop (M4B only; Save as… moves the file; ffmpeg help on Linux). */
export const DesktopExportPanel = (props: DesktopExportPanelProps) => {
  const [isConfirmingCancel, setIsConfirmingCancel] = useState(false);
  const confirmCancel = () => {
    setIsConfirmingCancel(false);
    props.onCancel();
  };
  return (
    <section className={styles.panel} aria-label="Export audiobook">
      <h2 className={styles.title}>Export audiobook</h2>
      {PHASE_VIEW[props.phase](props, () => setIsConfirmingCancel(true))}
      {isConfirmingCancel && <ConfirmDialog title="Cancel this export?" text="The parts made so far are deleted." cancelLabel="Keep exporting" confirmLabel="Cancel export" danger onCancel={() => setIsConfirmingCancel(false)} onConfirm={confirmCancel} />}
    </section>
  );
};

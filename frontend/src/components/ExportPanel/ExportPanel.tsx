import { EXPORT_FORMATS, EXPORT_FORMAT_EXTENSION, EXPORT_FORMAT_LABEL } from '../../config/constants';
import controls from '../../styles/controls.module.css';
import type { ExportFormat } from '../../types/api';
import type { ExportPhase } from '../../utils/exportPhase';
import styles from './ExportPanel.module.css';

interface ExportPanelProps {
  phase: ExportPhase;
  /** The format the next export will use. */
  format: ExportFormat;
  onFormatChange: (format: ExportFormat) => void;
  /** The finished (or latest) job's format and voice, for the download row. */
  fileFormat: ExportFormat;
  fileVoiceName: string;
  sentencesDone: number;
  sentencesTotal: number;
  /** From estimatedSecondsLeft; null if unknown. */
  timeLeftText: string | null;
  /** The voice the next export will use. */
  voiceName: string;
  readSkipped: boolean;
  fileUrl: string | null;
  downloadName: string;
  /** A request error (start, poll or cancel), separate from the job's own `failed` status. */
  errorMessage: string | null;
  /** The speech engine is ready. */
  canStart: boolean;
  onStart: () => void;
  onCancel: () => void;
}

const formatCount = (value: number): string => value.toLocaleString('en');

const FormatField = ({ format, onFormatChange, disabled }: Pick<ExportPanelProps, 'format' | 'onFormatChange'> & { disabled: boolean }) => (
  <label className={styles.field}>
    <span>Format</span>
    <select
      className={controls.select}
      value={format}
      disabled={disabled}
      onChange={(event) => {
        const chosen = EXPORT_FORMATS.find((candidate) => candidate === event.target.value);
        if (chosen) onFormatChange(chosen);
      }}
    >
      {EXPORT_FORMATS.map((candidate) => <option key={candidate} value={candidate}>{EXPORT_FORMAT_LABEL[candidate]}</option>)}
    </select>
  </label>
);

const StartControls = (props: ExportPanelProps) => (
  <>
    <div className={styles.row}>
      <button type="button" className={controls.button} disabled={!props.canStart} onClick={props.onStart}>{props.canStart ? 'Export whole book' : 'Needs the speech engine'}</button>
    </div>
    <p className={styles.help}>{props.voiceName} voice · code and tables: {props.readSkipped ? 'read aloud' : 'cue only'} · runs in the background while you listen.</p>
  </>
);

const CancelButton = ({ onCancel }: Pick<ExportPanelProps, 'onCancel'>) => <button type="button" className={controls.button} onClick={onCancel}>Cancel export</button>;

/** The format can't change while a job is being started or is running. */
const FORMAT_LOCKED: Record<ExportPhase, boolean> = { idle: false, starting: true, queued: true, running: true, done: false, failed: false, cancelled: false };

const PHASE_VIEW: Record<ExportPhase, (props: ExportPanelProps) => React.ReactNode> = {
  idle: (props) => <StartControls {...props} />,
  starting: () => (
    <div className={styles.row}>
      <button type="button" className={controls.button} disabled>Starting export…</button>
    </div>
  ),
  queued: (props) => (
    <>
      <p className={styles.text}>Waiting to start…</p>
      <progress className={styles.progress} aria-label="Export progress" />
      <div className={styles.row}><CancelButton onCancel={props.onCancel} /></div>
    </>
  ),
  running: (props) => (
    <>
      <progress className={styles.progress} aria-label="Export progress" value={props.sentencesDone} max={props.sentencesTotal} />
      <p className={styles.text}>{formatCount(props.sentencesDone)} of {formatCount(props.sentencesTotal)} sentences{props.timeLeftText ? ` · ${props.timeLeftText}` : ''}</p>
      <div className={styles.row}><CancelButton onCancel={props.onCancel} /></div>
    </>
  ),
  done: (props) => (
    <>
      <div className={styles.row}>
        <a className={`${controls.button} ${controls.primary}`} href={props.fileUrl ?? undefined} download={props.downloadName}>Download {EXPORT_FORMAT_EXTENSION[props.fileFormat].toUpperCase()}</a>
        <span className={styles.help}>Ready · {props.fileVoiceName} voice</span>
      </div>
      <div className={styles.row}><button type="button" className={controls.button} onClick={props.onStart}>Export again</button></div>
    </>
  ),
  failed: (props) => (
    <>
      <p className={styles.error}>The export stopped before it finished.</p>
      <div className={styles.row}><button type="button" className={controls.button} onClick={props.onStart}>Try again</button></div>
    </>
  ),
  cancelled: (props) => (
    <>
      <p className={styles.text}>Export cancelled.</p>
      <StartControls {...props} />
    </>
  ),
};

export const ExportPanel = (props: ExportPanelProps) => (
  <section className={styles.panel} aria-label="Export audio">
    <h2 className={styles.title}>Export audio</h2>
    <FormatField format={props.format} onFormatChange={props.onFormatChange} disabled={FORMAT_LOCKED[props.phase]} />
    {PHASE_VIEW[props.phase](props)}
    {props.errorMessage && <p className={styles.error} role="alert">{props.errorMessage}</p>}
  </section>
);

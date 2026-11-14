import { useAnnounced } from '../../../components/Announcer/useAnnounce';
import { CheckIcon, CloseIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import desktop from '../../styles/controls.module.css';
import { folderNameOf } from '../../utils/format';
import { WarningIcon } from '../icons';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import styles from './ExportToast.module.css';

interface ToastBase {
  bookTitle: string;
  /** The reader's chapters rail is open: the toast sits centred above the pill instead of bottom-left (S3-A). */
  railOpen: boolean;
  onDismiss: () => void;
}

export type ExportToastProps =
  | ({ kind: 'ready'; durationText: string; onSaveAs: () => void; onReveal: () => void } & ToastBase)
  | ({ kind: 'moving'; durationText: string } & ToastBase)
  | ({ kind: 'saved'; durationText: string; path: string; onReveal: () => void } & ToastBase)
  /** "Designing Calm Software: the disk is full." */
  | ({ kind: 'stopped'; reasonText: string; onOpenBook: () => void } & ToastBase)
  /** "Save as…" failed; the file is still in Lectrice. */
  | ({ kind: 'saveFailed'; storageFull: boolean; onSaveAs: () => void; onReveal: () => void } & ToastBase)
  /** The file was moved or deleted outside Lectrice. */
  | ({ kind: 'fileGone'; onOpenBook: () => void } & ToastBase)
  /** The file manager didn't start. */
  | ({ kind: 'revealFailed'; onReveal: () => void } & ToastBase);

type SaveFailure = 'full' | 'other';
const SAVE_FAILED_TITLE: Record<SaveFailure, string> = { full: 'Not enough space there', other: "Couldn't save" };
const SAVE_FAILED_PLACE: Record<SaveFailure, string> = { full: 'drive', other: 'folder' };

const Dismiss = ({ onDismiss }: Pick<ToastBase, 'onDismiss'>) => <button type="button" className={`${controls.iconButton} ${styles.dismiss}`} aria-label="Dismiss" onClick={onDismiss}><CloseIcon /></button>;
const ReadyTitle = ({ children }: { children: React.ReactNode }) => <span className={`${styles.title} ${desktop.ok}`}><CheckIcon /><span>{children}</span></span>;
const ErrorTitle = ({ children }: { children: React.ReactNode }) => <span className={`${styles.title} ${styles.error}`}><WarningIcon />{children}</span>;
const SaveButtons = ({ onSaveAs, onReveal }: { onSaveAs: () => void; onReveal: () => void }) => (
  <div className={desktop.actions}>
    <button type="button" className={`${controls.button} ${controls.primary}`} onClick={onSaveAs}>Save as…</button>
    <button type="button" className={controls.button} onClick={onReveal}>Show in folder</button>
  </div>
);
const OpenBookButton = ({ onOpenBook }: { onOpenBook: () => void }) => <div className={desktop.actions}><button type="button" className={controls.button} onClick={onOpenBook}>Open book</button></div>;

const READY_TITLE = 'Audiobook ready';

/** The card's first line, which is also what a screen reader hears when the card appears. */
const toastTitle = (props: ExportToastProps): string => {
  switch (props.kind) {
    case 'ready':
    case 'moving':
      return READY_TITLE;
    case 'saved':
      return `Saved to ${folderNameOf(props.path)}`;
    case 'stopped':
      return 'Export stopped';
    case 'saveFailed':
      return SAVE_FAILED_TITLE[props.storageFull ? 'full' : 'other'];
    case 'fileGone':
      return 'File not found';
    case 'revealFailed':
      return "Couldn't open the folder";
  }
};

/** What went well is announced politely; a stop or a failure interrupts. */
const IS_URGENT: Record<ExportToastProps['kind'], boolean> = { ready: false, moving: false, saved: false, stopped: true, saveFailed: true, fileGone: true, revealFailed: true };

const Body = ({ title, ...props }: ExportToastProps & { title: string }): React.ReactNode => {
  switch (props.kind) {
    case 'ready':
      return (
        <>
          <div className={styles.head}><ReadyTitle>{title}</ReadyTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <SaveButtons onSaveAs={props.onSaveAs} onReveal={props.onReveal} />
        </>
      );
    case 'moving':
      return (
        <>
          <div className={styles.head}><ReadyTitle>{title}</ReadyTitle></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <p>Moving…</p>
          <ProgressBar percent={null} label="Moving the file" size="small" />
        </>
      );
    case 'saved':
      return (
        <>
          <div className={styles.head}><ReadyTitle>{title}</ReadyTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <div className={desktop.actions}><button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onReveal}>Show in folder</button></div>
        </>
      );
    case 'stopped':
      return (
        <>
          <div className={styles.head}><ErrorTitle>{title}</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.reasonText}</p>
          <OpenBookButton onOpenBook={props.onOpenBook} />
        </>
      );
    case 'saveFailed':
      return (
        <>
          <div className={styles.head}><ErrorTitle>{title}</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle} is still in Lectrice. Try another {SAVE_FAILED_PLACE[props.storageFull ? 'full' : 'other']}.</p>
          <SaveButtons onSaveAs={props.onSaveAs} onReveal={props.onReveal} />
        </>
      );
    case 'fileGone':
      return (
        <>
          <div className={styles.head}><ErrorTitle>{title}</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle}: the file was moved or deleted.</p>
          <OpenBookButton onOpenBook={props.onOpenBook} />
        </>
      );
    case 'revealFailed':
      return (
        <>
          <div className={styles.head}><ErrorTitle>{title}</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle} · your file manager didn't start.</p>
          <div className={desktop.actions}><button type="button" className={controls.button} onClick={props.onReveal}>Show in folder</button></div>
        </>
      );
  }
};

/** UX3-C + S2-B: shown on any screen when an export of any book finishes, stops or can't be saved; stays until dismissed or acted on. */
export const ExportToast = (props: ExportToastProps) => {
  const title = toastTitle(props);
  useAnnounced(title, IS_URGENT[props.kind]);
  return <div className={`${styles.toast} ${props.railOpen ? styles.railOpen : ''}`}><Body {...props} title={title} /></div>;
};

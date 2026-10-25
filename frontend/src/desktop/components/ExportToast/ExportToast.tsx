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

const SAVE_FAILED_TEXT: Record<'full' | 'other', { title: string; place: string }> = {
  full: { title: 'Not enough space there', place: 'drive' },
  other: { title: "Couldn't save", place: 'folder' },
};

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

const Body = (props: ExportToastProps): React.ReactNode => {
  switch (props.kind) {
    case 'ready':
      return (
        <>
          <div className={styles.head}><ReadyTitle>Audiobook ready</ReadyTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <SaveButtons onSaveAs={props.onSaveAs} onReveal={props.onReveal} />
        </>
      );
    case 'moving':
      return (
        <>
          <div className={styles.head}><ReadyTitle>Audiobook ready</ReadyTitle></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <p>Moving…</p>
          <ProgressBar percent={null} label="Moving the file" size="small" />
        </>
      );
    case 'saved':
      return (
        <>
          <div className={styles.head}><ReadyTitle>Saved to {folderNameOf(props.path)}</ReadyTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p className={desktop.meta}>{props.bookTitle} · {props.durationText}</p>
          <div className={desktop.actions}><button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onReveal}>Show in folder</button></div>
        </>
      );
    case 'stopped':
      return (
        <>
          <div className={styles.head}><ErrorTitle>Export stopped</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.reasonText}</p>
          <OpenBookButton onOpenBook={props.onOpenBook} />
        </>
      );
    case 'saveFailed': {
      const text = SAVE_FAILED_TEXT[props.storageFull ? 'full' : 'other'];
      return (
        <>
          <div className={styles.head}><ErrorTitle>{text.title}</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle} is still in Lectrice. Try another {text.place}.</p>
          <SaveButtons onSaveAs={props.onSaveAs} onReveal={props.onReveal} />
        </>
      );
    }
    case 'fileGone':
      return (
        <>
          <div className={styles.head}><ErrorTitle>File not found</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle}: the file was moved or deleted.</p>
          <OpenBookButton onOpenBook={props.onOpenBook} />
        </>
      );
    case 'revealFailed':
      return (
        <>
          <div className={styles.head}><ErrorTitle>Couldn't open the folder</ErrorTitle><Dismiss onDismiss={props.onDismiss} /></div>
          <p>{props.bookTitle} · your file manager didn't start.</p>
          <div className={desktop.actions}><button type="button" className={controls.button} onClick={props.onReveal}>Show in folder</button></div>
        </>
      );
  }
};

/** UX3-C + S2-B: shown on any screen when an export of any book finishes, stops or can't be saved; stays until dismissed or acted on. */
export const ExportToast = (props: ExportToastProps) => (
  <div className={`${styles.toast} ${props.railOpen ? styles.railOpen : ''}`} role="status"><Body {...props} /></div>
);

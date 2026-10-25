import { useId, useLayoutEffect, useRef } from 'react';
import { CloseIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import desktop from '../../styles/controls.module.css';
import styles from './ModalDialog.module.css';

interface ModalDialogProps {
  title: string;
  /** Hides the ✕; the footer's buttons are then the only way out besides Escape. */
  showClose?: boolean;
  size?: 'small' | 'wide';
  /** Escape, ✕ or a footer button: the parent unmounts the dialog. */
  onClose: () => void;
  children: React.ReactNode;
  footer?: React.ReactNode;
}

/** A native modal (`showModal()`: focus trap, Escape, backdrop) in the app's look; buttons right-aligned, primary last. */
export const ModalDialog = ({ title, showClose = true, size = 'small', onClose, children, footer }: ModalDialogProps) => {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    dialog.showModal();
    return () => dialog.close();
  }, []);

  // Escape fires `cancel`; letting the dialog close itself would leave React believing it is open.
  const handleCancel = (event: React.SyntheticEvent) => {
    event.preventDefault();
    onClose();
  };

  return (
    <dialog ref={dialogRef} className={size === 'wide' ? `${styles.dialog} ${styles.wide}` : styles.dialog} aria-labelledby={titleId} onCancel={handleCancel}>
      <div className={styles.head}>
        <h2 id={titleId} className={styles.title}>{title}</h2>
        {showClose && <button type="button" className={`${controls.iconButton} ${styles.close}`} aria-label="Close" onClick={onClose}><CloseIcon /></button>}
      </div>
      <div className={styles.body}>{children}</div>
      {footer && <div className={styles.footer}>{footer}</div>}
    </dialog>
  );
};

interface ConfirmDialogProps {
  title: string;
  text: string;
  cancelLabel: string;
  confirmLabel: string;
  /** The confirm button in the error look (Delete, Cancel export). */
  danger?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

/** A question with two buttons; the safe one comes first and the primary one last. */
export const ConfirmDialog = ({ title, text, cancelLabel, confirmLabel, danger = false, onCancel, onConfirm }: ConfirmDialogProps) => (
  <ModalDialog
    title={title}
    showClose={false}
    onClose={onCancel}
    footer={(
      <>
        <button type="button" className={controls.button} onClick={onCancel}>{cancelLabel}</button>
        <button type="button" className={`${controls.button} ${danger ? desktop.dangerButton : controls.primary}`} onClick={onConfirm}>{confirmLabel}</button>
      </>
    )}
  >
    <p>{text}</p>
  </ModalDialog>
);

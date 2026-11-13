import { CloseIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { PhoneHelperStatus } from '../../commands/types';
import { versionToastText } from '../../utils/phoneHelper';
import toast from '../ExportToast/ExportToast.module.css';
import { WarningIcon } from '../icons';

interface PhoneVersionToastProps {
  mismatch: NonNullable<PhoneHelperStatus['versionMismatch']>;
  /** The reader's chapters rail is open: the toast sits centred above the pill, like the export toast. */
  railOpen: boolean;
  onDismiss: () => void;
}

/** P4: the phone and this computer run versions that can't work together; says which one to update. */
export const PhoneVersionToast = ({ mismatch, railOpen, onDismiss }: PhoneVersionToastProps) => {
  const text = versionToastText(mismatch);
  return (
    <div className={`${toast.toast} ${railOpen ? toast.railOpen : ''}`} role="status">
      <div className={toast.head}>
        <span className={`${toast.title} ${toast.error}`}><WarningIcon />{text.title}</span>
        <button type="button" className={`${controls.iconButton} ${toast.dismiss}`} aria-label="Dismiss" onClick={onDismiss}><CloseIcon /></button>
      </div>
      <p>{text.body}</p>
    </div>
  );
};

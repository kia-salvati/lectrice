import { useAnnounced } from '../../../components/Announcer/useAnnounce';
import { CloseIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { CommandFailure } from '../../hooks/useCommandErrors';
import toast from '../ExportToast/ExportToast.module.css';
import { WarningIcon } from '../icons';

interface CommandErrorToastProps {
  failure: CommandFailure;
  /** The reader's chapters rail is open: the toast sits centred above the pill, like the export toast. */
  railOpen: boolean;
  onDismiss: () => void;
}

/** Something the user asked for didn't happen: what, and the app's reason. Stays until dismissed. */
export const CommandErrorToast = ({ failure, railOpen, onDismiss }: CommandErrorToastProps) => {
  useAnnounced(failure.title, true);
  return (
    <div className={`${toast.toast} ${railOpen ? toast.railOpen : ''}`}>
      <div className={toast.head}>
        <span className={`${toast.title} ${toast.error}`}><WarningIcon />{failure.title}</span>
        <button type="button" className={`${controls.iconButton} ${toast.dismiss}`} aria-label="Dismiss" onClick={onDismiss}><CloseIcon /></button>
      </div>
      <p>{failure.message}</p>
    </div>
  );
};

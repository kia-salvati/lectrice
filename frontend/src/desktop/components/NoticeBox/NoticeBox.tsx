import { InfoIcon, WarningIcon } from '../icons';
import styles from './NoticeBox.module.css';

type NoticeKind = 'info' | 'error';

interface NoticeBoxProps { kind: NoticeKind; children: React.ReactNode; }

const LOOK: Record<NoticeKind, { className: string; role: string; icon: React.ReactNode }> = {
  info: { className: styles.info, role: 'note', icon: <InfoIcon /> },
  error: { className: styles.error, role: 'alert', icon: <WarningIcon /> },
};

/** An info box (`bg2`, info icon) or an error box (`err-bg`, warning icon); children are the paragraphs. */
export const NoticeBox = ({ kind, children }: NoticeBoxProps) => {
  const look = LOOK[kind];
  return (
    <div className={`${styles.box} ${look.className}`} role={look.role}>
      {look.icon}
      <div className={styles.body}>{children}</div>
    </div>
  );
};

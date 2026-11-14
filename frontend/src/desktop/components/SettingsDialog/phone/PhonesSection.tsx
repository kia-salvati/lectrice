import { useId } from 'react';
import type { PairedPhone, PhoneHelperStatus } from '../../../commands/types';
import { RELATIVE_TIME_TICK_MS } from '../../../config';
import { useNow } from '../../../hooks/useNow';
import desktop from '../../../styles/controls.module.css';
import { phoneMetaText } from '../../../utils/phoneHelper';
import styles from '../PhoneTab.module.css';

interface PhonesSectionProps {
  status: PhoneHelperStatus;
  /** The phone that has just paired: its row arrives highlighted. */
  newPeerId: string | null;
  onForget: (phone: PairedPhone) => void;
}

/** The paired phones, each with what it is doing or when it was last seen. */
export const PhonesSection = ({ status, newPeerId, onForget }: PhonesSectionProps) => {
  const headingId = useId();
  const now = useNow(RELATIVE_TIME_TICK_MS);
  return (
    <section className={styles.section} aria-labelledby={headingId}>
      <h3 id={headingId} className={styles.heading}>Phones</h3>
      {status.phones.length === 0 ? <p className={desktop.muted}>No phones paired yet.</p> : (
        <ul className={styles.phones}>
          {status.phones.map((phone) => (
            <li key={phone.peerId} className={phone.peerId === newPeerId ? `${styles.phone} ${styles.arrived}` : styles.phone}>
              <span className={styles.phoneText}>
                <span className={styles.phoneName}>{phone.name}</span>
                <span className={desktop.meta}>{phoneMetaText(phone, status.enabled, now)}</span>
              </span>
              <button type="button" className={desktop.textButton} onClick={() => onForget(phone)}>Forget</button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

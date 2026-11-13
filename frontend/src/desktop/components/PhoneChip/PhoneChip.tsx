import type { PairedPhone } from '../../commands/types';
import { chipText, makingAudioText } from '../../utils/phoneHelper';
import { PhoneIcon } from '../icons';
import styles from './PhoneChip.module.css';

interface PhoneMakingProps {
  /** The phones with audio in the making; nothing is drawn while there are none. */
  making: readonly PairedPhone[];
  /** Opens the settings on the Phone tab. */
  onOpen: () => void;
}

/** P2-A: the library header's chip, only while this computer makes audio for a phone. */
export const PhoneChip = ({ making, onOpen }: PhoneMakingProps) => {
  if (making.length === 0) return null;
  const title = makingAudioText(making);
  return <button type="button" className={styles.chip} title={title} aria-label={title} onClick={onOpen}><PhoneIcon /><span>{chipText(making)}</span></button>;
};

/** P2-A: the same news as one line under the reader popover's buttons. */
export const PhoneMakingLine = ({ making, onOpen }: PhoneMakingProps) => {
  if (making.length === 0) return null;
  return <button type="button" className={styles.line} onClick={onOpen}><PhoneIcon /><span>{makingAudioText(making)}</span></button>;
};

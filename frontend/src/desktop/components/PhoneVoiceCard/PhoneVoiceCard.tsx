import { useAnnounced } from '../../../components/Announcer/useAnnounce';
import controls from '../../../styles/controls.module.css';
import type { PhoneModelRequest } from '../../commands/types';
import desktop from '../../styles/controls.module.css';
import { formatMb } from '../../utils/format';
import styles from './PhoneVoiceCard.module.css';

interface PhoneVoiceCardProps {
  request: PhoneModelRequest;
  onDownload: () => void;
  onNotNow: () => void;
}

/** P3: a phone uses a voice this computer doesn't have; shown on any screen, bottom centre above the pill. */
export const PhoneVoiceCard = ({ request, onDownload, onNotNow }: PhoneVoiceCardProps) => {
  const question = `Your phone uses the ${request.displayName} voice. Download it here (${formatMb(request.downloadBytes)}) so this computer can help?`;
  useAnnounced(question);
  return (
    <div className={styles.card}>
      <p>{question}</p>
      <div className={desktop.actions}>
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={onDownload}>Download</button>
        <button type="button" className={desktop.textButton} onClick={onNotNow}>Not now</button>
      </div>
    </div>
  );
};

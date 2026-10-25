import controls from '../../../styles/controls.module.css';
import type { ModelStatus, ModelTransfer } from '../../commands/types';
import desktop from '../../styles/controls.module.css';
import { formatProgressMb, percentOf } from '../../utils/format';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import styles from './VoiceStrip.module.css';

interface VoiceStripProps {
  status: ModelStatus;
  /** "Not now" was chosen earlier: the strip offers the setup screen again. */
  onGetVoice: () => void;
  onResume: () => void;
  /** Back to the setup column, which shows the download in full. */
  onDetails: () => void;
}

type ProgressingTransfer = Extract<ModelTransfer, { doneBytes: number }>;

const DetailsButton = ({ onDetails }: Pick<VoiceStripProps, 'onDetails'>) => <button type="button" className={desktop.textButton} onClick={onDetails}>Details</button>;

/** "Downloading the voice · 41 %" with the small bar; `transfer` null while nothing is measurable yet. */
const Downloading = ({ text, transfer, onDetails }: { text: string; transfer: ProgressingTransfer | null } & Pick<VoiceStripProps, 'onDetails'>) => (
  <div className={styles.strip}>
    <span className={styles.left}>
      <span>Downloading the voice · {text}</span>
      <span className={styles.bar}>
        <ProgressBar percent={transfer && percentOf(transfer.doneBytes, transfer.totalBytes)} valueText={transfer ? formatProgressMb(transfer.doneBytes, transfer.totalBytes) : undefined} label="Voice download" size="small" />
      </span>
    </span>
    <span className={desktop.actions}><DetailsButton onDetails={onDetails} /></span>
  </div>
);

const percentText = (transfer: ProgressingTransfer): string => `${percentOf(transfer.doneBytes, transfer.totalBytes)} %`;

const TransferStrip = ({ transfer, onResume, onDetails }: { transfer: Exclude<ModelTransfer, { kind: 'none' }> } & Pick<VoiceStripProps, 'onResume' | 'onDetails'>) => {
  switch (transfer.kind) {
    case 'starting':
      return <Downloading text="Starting…" transfer={null} onDetails={onDetails} />;
    case 'verifying':
      return <Downloading text="Checking…" transfer={null} onDetails={onDetails} />;
    case 'downloading':
    case 'retrying':
    case 'waitingForNetwork':
      return <Downloading text={percentText(transfer)} transfer={transfer} onDetails={onDetails} />;
    case 'paused':
      return (
        <div className={styles.strip}>
          <span>Voice download paused · {percentText(transfer)}</span>
          <span className={desktop.actions}>
            <button type="button" className={controls.button} onClick={onResume}>Resume</button>
            <DetailsButton onDetails={onDetails} />
          </span>
        </div>
      );
    case 'failed':
      return (
        <div className={`${styles.strip} ${styles.error}`} role="alert">
          <span>The voice download stopped.</span>
          <span className={desktop.actions}><DetailsButton onDetails={onDetails} /></span>
        </div>
      );
  }
};

/** Under the library header while the voice is missing or on its way; nothing once a bundle is installed. */
export const VoiceStrip = ({ status, onGetVoice, onResume, onDetails }: VoiceStripProps) => {
  if (status.transfer.kind !== 'none') return <TransferStrip transfer={status.transfer} onResume={onResume} onDetails={onDetails} />;
  if (status.bundles.some((bundle) => bundle.installed)) return null;
  return (
    <div className={styles.strip}>
      <span>No voice yet: Lectrice can show your PDFs but can't read them aloud.</span>
      <span className={desktop.actions}><button type="button" className={controls.button} onClick={onGetVoice}>Get the voice</button></span>
    </div>
  );
};

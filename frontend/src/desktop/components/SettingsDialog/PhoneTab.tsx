import { useEffect, useId, useRef, useState } from 'react';
import { CheckIcon, WaitIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { PairedPhone, PhoneHelperStatus } from '../../commands/types';
import { PAIRED_CARD_CLOSES_MS } from '../../config';
import type { PhoneHelperActions } from '../../hooks/usePhoneHelper';
import desktop from '../../styles/controls.module.css';
import { otherNetworkText } from '../../utils/phoneHelper';
import { CopyIconButton } from '../CopyIconButton/CopyIconButton';
import { ConfirmDialog } from '../ModalDialog/ModalDialog';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { PairingCard } from '../PairingCard/PairingCard';
import { AdvancedSection } from './phone/AdvancedSection';
import { ComputerName } from './phone/ComputerName';
import { PhonesSection } from './phone/PhonesSection';
import { Switch } from './phone/Switch';
import styles from './PhoneTab.module.css';

interface PhoneTabProps {
  status: PhoneHelperStatus;
  actions: PhoneHelperActions;
}

/** Whether this computer can be reached, and what to do when it can't. */
const ListeningStatus = ({ status }: Pick<PhoneTabProps, 'status'>) => {
  const { notice, listening } = status;
  if (listening === 'failed' || notice?.kind === 'portInUse') {
    return <p className={desktop.errorText}>Port {notice?.kind === 'portInUse' ? notice.port : status.port} is in use. Choose another port under Advanced.</p>;
  }
  if (notice?.kind === 'noNetwork' || status.network.kind === 'none') return <p>This computer isn't on a home network right now.</p>;
  if (notice?.kind === 'linuxFirewall') {
    return (
      <NoticeBox kind="info">
        <p>Your firewall may block your phone. To allow it, run:</p>
        {notice.commands.map((command) => (
          <div key={command} className={styles.command}><code className={styles.code}>{command}</code><CopyIconButton text={command} /></div>
        ))}
      </NoticeBox>
    );
  }
  if (listening === 'starting') return <p className={`${styles.statusLine} ${desktop.spin}`}><WaitIcon /><span>Starting…</span></p>;
  if (listening === 'ready') return <p><span className={desktop.ok}><CheckIcon /><span>Ready for your phone</span></span></p>;
  return null;
};

const ComputerSection = ({ status, actions }: PhoneTabProps) => {
  const headingId = useId();
  const { network } = status;
  const [address, ...otherAddresses] = status.addresses;
  return (
    <section className={styles.section} aria-labelledby={headingId}>
      <h3 id={headingId} className={styles.heading}>This computer</h3>
      <div className={styles.facts}>
        <span className={desktop.muted}>Name</span>
        <span className={styles.value}><ComputerName name={status.computerName} onRename={actions.rename} /></span>
        {network.kind !== 'other' && address !== undefined && (
          <>
            <span className={desktop.muted}>Address</span>
            <span className={styles.value}><span className={styles.mono}>{address}</span><CopyIconButton text={address} /></span>
            {otherAddresses.length > 0 && <span className={`${desktop.meta} ${styles.also}`}>Also: {otherAddresses.join(', ')}</span>}
          </>
        )}
      </div>
      {network.kind === 'other' ? (
        <NoticeBox kind="info">
          <p>{otherNetworkText(network.name)}</p>
          <div><button type="button" className={controls.button} onClick={actions.allowThisNetwork}>Use on this network too</button></div>
        </NoticeBox>
      ) : <ListeningStatus status={status} />}
    </section>
  );
};

/** P1-A: the settings dialog's Phone tab; the pairing card opens in the button's place, under the list of phones. */
export const PhoneTab = ({ status, actions }: PhoneTabProps) => {
  const switchId = useId();
  const cardRef = useRef<HTMLDivElement>(null);
  const [phoneToForget, setPhoneToForget] = useState<PairedPhone | null>(null);
  /** The phone whose "Paired with …" card has closed by itself. */
  const [closedPairedPeerId, setClosedPairedPeerId] = useState<string | null>(null);
  const { pairing, network } = status;
  const pairedPeerId = pairing.kind === 'paired' ? pairing.peerId : null;

  if (pairedPeerId === null && closedPairedPeerId !== null) setClosedPairedPeerId(null);

  useEffect(() => {
    if (pairedPeerId === null) return;
    const timer = setTimeout(() => setClosedPairedPeerId(pairedPeerId), PAIRED_CARD_CLOSES_MS);
    return () => clearTimeout(timer);
  }, [pairedPeerId]);

  const cardOpen = status.enabled && pairing.kind !== 'none' && (pairedPeerId === null || pairedPeerId !== closedPairedPeerId);

  // The card opens under the list and grows when a phone starts trying: keep all of it in view.
  const cardShape = cardOpen ? `${pairing.kind}/${pairing.kind === 'showing' ? pairing.trying.length : 0}` : null;
  useEffect(() => {
    if (cardShape !== null) cardRef.current?.scrollIntoView({ block: 'nearest' });
  }, [cardShape]);

  return (
    <div className={styles.tab}>
      <div className={styles.switchRow}>
        <div className={styles.switchLine}><label htmlFor={switchId} className={styles.switchLabel}>Let my phone use this computer</label><Switch id={switchId} checked={status.enabled} onChange={actions.setEnabled} /></div>
        <p className={styles.explanation}>Your phone sends the sentences it is about to read; this computer turns them into speech and sends the audio back. Nothing leaves your home network, and nothing is saved here.</p>
        {status.enabled && network.kind === 'allowed' && network.name !== null && <p className={desktop.meta}>On {network.name}</p>}
      </div>
      {status.notice?.kind === 'windowsFirstListen' && (
        <NoticeBox kind="info"><p>Windows will ask whether Lectrice may use your network. Choose <b>Private networks</b>.</p></NoticeBox>
      )}
      {status.enabled && <ComputerSection status={status} actions={actions} />}
      <PhonesSection status={status} newPeerId={pairedPeerId} onForget={setPhoneToForget} />
      {pairing.kind !== 'none' && cardOpen && (
        <div ref={cardRef}><PairingCard pairing={pairing} computerName={status.computerName} onStart={actions.pairStart} onCancel={actions.pairCancel} /></div>
      )}
      {/* Pairing needs a listening computer on an allowed network (a phone couldn't reach it otherwise). */}
      {status.enabled && status.listening === 'ready' && network.kind === 'allowed' && !cardOpen && <div><button type="button" className={`${controls.button} ${controls.primary}`} onClick={actions.pairStart}>Pair a phone</button></div>}
      <AdvancedSection status={status} actions={actions} />
      <p className={desktop.meta}>On your phone: Settings → Computer helper. Both must be on the same Wi-Fi, not a guest network.</p>
      {phoneToForget && (
        <ConfirmDialog
          title={`Forget ${phoneToForget.name}?`}
          text="It will have to pair again to use this computer."
          cancelLabel="Cancel"
          confirmLabel="Forget"
          danger
          onCancel={() => setPhoneToForget(null)}
          onConfirm={() => {
            actions.forget(phoneToForget.peerId);
            setPhoneToForget(null);
          }}
        />
      )}
    </div>
  );
};

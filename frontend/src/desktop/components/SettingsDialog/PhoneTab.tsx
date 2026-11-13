import { useEffect, useId, useRef, useState } from 'react';
import { CheckIcon, WaitIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { LinkId, PairedPhone, PhoneHelperStatus } from '../../commands/types';
import { COMPUTER_NAME_MAX_LENGTH, PAIRED_CARD_CLOSES_MS, RELATIVE_TIME_TICK_MS } from '../../config';
import type { PhoneHelperActions } from '../../hooks/usePhoneHelper';
import { useNow } from '../../hooks/useNow';
import desktop from '../../styles/controls.module.css';
import { otherNetworkText, parsePort, phoneMetaText, PORT_MAX, PORT_MIN } from '../../utils/phoneHelper';
import { CopyIconButton } from '../CopyIconButton/CopyIconButton';
import { ConfirmDialog } from '../ModalDialog/ModalDialog';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { PairingCard } from '../PairingCard/PairingCard';
import styles from './PhoneTab.module.css';

interface PhoneTabProps {
  status: PhoneHelperStatus;
  actions: PhoneHelperActions;
  onOpenLink: (linkId: LinkId) => void;
}

interface SwitchProps { id: string; checked: boolean; onChange: (checked: boolean) => void; }

const Switch = ({ id, checked, onChange }: SwitchProps) => (
  <input type="checkbox" role="switch" id={id} className={styles.switch} checked={checked} onChange={(event) => onChange(event.target.checked)} />
);

/** This computer's name: a text with "Rename", or the input (Enter saves, Escape or leaving the field cancels). */
const ComputerName = ({ name, onRename }: { name: string; onRename: (name: string) => void }) => {
  const [draft, setDraft] = useState<string | null>(null);
  if (draft === null) {
    return <><span>{name}</span><button type="button" className={desktop.textButton} onClick={() => setDraft(name)}>Rename</button></>;
  }
  const handleKey = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      // Escape would also close the settings dialog.
      event.preventDefault();
      event.stopPropagation();
      setDraft(null);
    } else if (event.key === 'Enter') {
      const trimmed = draft.trim();
      if (trimmed !== '' && trimmed !== name) onRename(trimmed);
      setDraft(null);
    }
  };
  return (
    <input
      type="text"
      className={styles.field}
      aria-label="This computer's name"
      maxLength={COMPUTER_NAME_MAX_LENGTH}
      value={draft}
      autoFocus
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={handleKey}
      onBlur={() => setDraft(null)}
    />
  );
};

/** Whether this computer can be reached, and what to do when it can't. */
const ListeningStatus = ({ status, onOpenLink }: Pick<PhoneTabProps, 'status' | 'onOpenLink'>) => {
  const { notice, listening } = status;
  if (listening === 'failed' || notice?.kind === 'portInUse') {
    return <p className={desktop.errorText}>Port {notice?.kind === 'portInUse' ? notice.port : status.port} is in use. Choose another port under Advanced.</p>;
  }
  if (notice?.kind === 'noNetwork' || status.network.kind === 'none') return <p>This computer isn't on a home network right now.</p>;
  if (notice?.kind === 'windowsPublicNetwork') {
    return (
      <NoticeBox kind="warning">
        <p>Windows treats this Wi-Fi as a public network, so your phone can't reach this computer. Set it to Private in Settings → Network &amp; internet → Wi-Fi → (your network).</p>
        <div><button type="button" className={controls.button} onClick={() => onOpenLink('windowsNetworkSettings')}>Open network settings</button></div>
      </NoticeBox>
    );
  }
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

const ComputerSection = ({ status, actions, onOpenLink }: PhoneTabProps) => {
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
      ) : <ListeningStatus status={status} onOpenLink={onOpenLink} />}
    </section>
  );
};

interface PhonesSectionProps {
  status: PhoneHelperStatus;
  /** The phone that has just paired: its row arrives highlighted. */
  newPeerId: string | null;
  onForget: (phone: PairedPhone) => void;
}

const PhonesSection = ({ status, newPeerId, onForget }: PhonesSectionProps) => {
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

const AdvancedSection = ({ status, actions }: Pick<PhoneTabProps, 'status' | 'actions'>) => {
  const voicesId = useId();
  const portId = useId();
  /** Null while the field shows the saved port. */
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [portRefused, setPortRefused] = useState(false);
  const savedPort = String(status.port);
  const changed = portDraft !== null && portDraft !== savedPort;

  const savePort = () => {
    const port = parsePort(portDraft ?? savedPort);
    if (port === null) {
      setPortRefused(true);
      return;
    }
    actions.setPort(port);
    setPortDraft(null);
  };

  return (
    <details className={styles.advanced}>
      <summary>Advanced</summary>
      <div className={styles.advancedBody}>
        <div>
          <div className={styles.switchLine}><label htmlFor={voicesId}>Download voices my phone needs</label><Switch id={voicesId} checked={status.downloadVoicesForPhone} onChange={actions.setDownloadVoices} /></div>
          <p className={desktop.meta}>Download without asking when your phone uses a voice this computer doesn't have.</p>
        </div>
        <div>
          <div className={styles.portRow}>
            <label htmlFor={portId}>Port</label>
            <input
              id={portId}
              type="number"
              className={`${styles.field} ${styles.port}`}
              min={PORT_MIN}
              max={PORT_MAX}
              value={portDraft ?? savedPort}
              onChange={(event) => {
                setPortDraft(event.target.value);
                setPortRefused(false);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && changed) savePort();
              }}
            />
            {changed && <button type="button" className={controls.button} onClick={savePort}>Save</button>}
          </div>
          {portRefused && <p className={desktop.errorText} role="alert">Use a number from {PORT_MIN} to {PORT_MAX}.</p>}
        </div>
      </div>
    </details>
  );
};

/** P1-A: the settings dialog's Phone tab; the pairing card opens in the button's place, under the list of phones. */
export const PhoneTab = ({ status, actions, onOpenLink }: PhoneTabProps) => {
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
      {status.enabled && <ComputerSection status={status} actions={actions} onOpenLink={onOpenLink} />}
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

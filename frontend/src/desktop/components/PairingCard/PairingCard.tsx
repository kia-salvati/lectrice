import { WaitIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { PhonePairing } from '../../commands/types';
import { PAIRING_TICK_MS, RELATIVE_TIME_TICK_MS } from '../../config';
import { useNow } from '../../hooks/useNow';
import desktop from '../../styles/controls.module.css';
import { codeAriaLabel, coolDownMinutes, countdownPercent, formatCountdown, groupCode, tryingText, WRONG_CODES_ALLOWED, wrongCodesText } from '../../utils/phoneHelper';
import { DoneIcon, WarningIcon } from '../icons';
import styles from './PairingCard.module.css';

type OpenPairing = Exclude<PhonePairing, { kind: 'none' }>;

interface PairingCardProps {
  pairing: OpenPairing;
  computerName: string;
  /** "New code" and "Pair a phone": only a click opens a new code. */
  onStart: () => void;
  onCancel: () => void;
}

const CancelButton = ({ onCancel }: Pick<PairingCardProps, 'onCancel'>) => <button type="button" className={`${desktop.textButton} ${styles.cancel}`} onClick={onCancel}>Cancel</button>;

const Showing = ({ pairing, computerName, onCancel }: { pairing: Extract<PhonePairing, { kind: 'showing' }> } & Pick<PairingCardProps, 'computerName' | 'onCancel'>) => {
  const msLeft = pairing.expiresAt - useNow(PAIRING_TICK_MS);
  return (
    <>
      <h3 className={styles.heading}>Pair a phone</h3>
      <div aria-live="polite">{pairing.wrongCodes > 0 && <p className={desktop.errorText}>{wrongCodesText(pairing.wrongCodes)}</p>}</div>
      <div className={styles.code} role="img" aria-label={codeAriaLabel(pairing.code)}>{groupCode(pairing.code)}</div>
      <div className={styles.countdown}>
        <div className={styles.bar} aria-hidden="true"><div className={styles.fill} style={{ width: `${countdownPercent(msLeft, pairing.totalSeconds)}%` }} /></div>
        <p className={desktop.meta}>Code for {computerName} · {formatCountdown(msLeft)} left</p>
      </div>
      <p>On your phone: Settings → Computer helper → Pair with a computer, then type this code.</p>
      {pairing.trying.length > 0 && <p className={`${styles.line} ${desktop.spin}`}><WaitIcon /><span>{tryingText(pairing.trying)}</span></p>}
      <CancelButton onCancel={onCancel} />
    </>
  );
};

const CoolingDown = ({ until, onCancel }: { until: number } & Pick<PairingCardProps, 'onCancel'>) => {
  const now = useNow(RELATIVE_TIME_TICK_MS);
  return (
    <>
      <p>Too many wrong codes this hour. You can pair again in {coolDownMinutes(until, now)} min.</p>
      <CancelButton onCancel={onCancel} />
    </>
  );
};

const Body = ({ pairing, computerName, onStart, onCancel }: PairingCardProps): React.ReactNode => {
  switch (pairing.kind) {
    case 'showing':
      return <Showing pairing={pairing} computerName={computerName} onCancel={onCancel} />;
    case 'paired':
      return <p className={styles.done} aria-live="polite"><DoneIcon /><span>Paired with {pairing.phoneName}</span></p>;
    case 'expired':
      return (
        <>
          <p>The code expired.</p>
          <div className={desktop.actions}>
            <button type="button" className={`${controls.button} ${controls.primary}`} onClick={onStart}>New code</button>
            <CancelButton onCancel={onCancel} />
          </div>
        </>
      );
    case 'tooManyWrongCodes':
      return (
        <>
          <p className={`${styles.line} ${desktop.errorText}`}><WarningIcon /><span>{`Someone typed a wrong code ${WRONG_CODES_ALLOWED} times, so pairing stopped.`}</span></p>
          {pairing.lastFrom !== null && <p className={desktop.meta}>Last try from {pairing.lastFrom}.</p>}
          <div className={desktop.actions}>
            <button type="button" className={`${controls.button} ${controls.primary}`} onClick={onStart}>Pair a phone</button>
            <CancelButton onCancel={onCancel} />
          </div>
        </>
      );
    case 'coolingDown':
      return <CoolingDown until={pairing.until} onCancel={onCancel} />;
  }
};

/** P1-A: the pairing code, its countdown and who is trying; a wrong code never brings a new code by itself. */
export const PairingCard = (props: PairingCardProps) => (
  <section className={styles.card} aria-label="Pair a phone"><Body {...props} /></section>
);

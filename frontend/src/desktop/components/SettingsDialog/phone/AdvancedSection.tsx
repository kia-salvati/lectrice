import { useId, useState } from 'react';
import controls from '../../../../styles/controls.module.css';
import type { PhoneHelperStatus } from '../../../commands/types';
import type { PhoneHelperActions } from '../../../hooks/usePhoneHelper';
import desktop from '../../../styles/controls.module.css';
import { parsePort, PORT_MAX, PORT_MIN } from '../../../utils/phoneHelper';
import styles from '../PhoneTab.module.css';
import { Switch } from './Switch';

/** Closed by default: downloading the phone's voices without asking, and the port. */
export const AdvancedSection = ({ status, actions }: { status: PhoneHelperStatus; actions: PhoneHelperActions }) => {
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

import controls from '../../../styles/controls.module.css';
import type { LinkId } from '../../commands/types';
import type { PhoneHelper } from '../../hooks/usePhoneHelper';
import desktop from '../../styles/controls.module.css';
import { PhoneTab } from './PhoneTab';

interface PhoneTabStateProps { phone: PhoneHelper; onOpenLink: (linkId: LinkId) => void; }

/** The Phone tab once the shell has answered; before that a quiet line, or a way to ask again. */
export const PhoneTabState = ({ phone, onOpenLink }: PhoneTabStateProps) => {
  if (phone.status !== null) return <PhoneTab status={phone.status} actions={phone.actions} onOpenLink={onOpenLink} />;
  if (!phone.loadFailed) return <p className={desktop.muted} role="status">Loading…</p>;
  return (
    <>
      <p className={desktop.errorText}>The phone settings didn't load.</p>
      <div><button type="button" className={controls.button} onClick={phone.retryLoad}>Try again</button></div>
    </>
  );
};

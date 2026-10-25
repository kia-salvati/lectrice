import controls from '../../../styles/controls.module.css';
import desktop from '../../styles/controls.module.css';

/** In the pill popover, where the Voice menu would be, while no voice is installed. */
export const NoVoiceRow = ({ onGetVoice }: { onGetVoice: () => void }) => (
  <div className={desktop.actions}>
    <span>No voice yet.</span>
    <button type="button" className={controls.button} onClick={onGetVoice}>Get the voice…</button>
  </div>
);

import { PauseIcon, PlayIcon, WaitIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import { SAMPLE_LINE } from '../../config';
import desktop from '../../styles/controls.module.css';
import styles from './SampleCard.module.css';

/** `engineLoading`: the voice itself is still starting, so the button only shows a spinner. */
export type SampleState = 'idle' | 'loading' | 'playing' | 'engineLoading';

interface SampleCardProps {
  /** "Heart · American English" */
  voiceLabel: string;
  state: SampleState;
  onToggle: () => void;
}

interface SampleButtonLook { icon: React.ReactNode; label: string; busy: boolean; disabled: boolean; }

const BUTTON: Record<SampleState, SampleButtonLook> = {
  idle: { icon: <PlayIcon />, label: 'Play sample', busy: false, disabled: false },
  loading: { icon: <WaitIcon />, label: 'Play sample', busy: true, disabled: false },
  playing: { icon: <PauseIcon />, label: 'Stop sample', busy: false, disabled: false },
  engineLoading: { icon: <WaitIcon />, label: 'Starting the voice', busy: true, disabled: true },
};

export const SampleCard = ({ voiceLabel, state, onToggle }: SampleCardProps) => {
  const look = BUTTON[state];
  const className = [controls.iconButton, controls.play, look.busy ? desktop.spin : ''].join(' ');
  return (
    <div className={styles.card}>
      <button type="button" className={className} aria-label={look.label} aria-busy={look.busy} disabled={look.disabled} onClick={onToggle}>{look.icon}</button>
      <div className={styles.text}>
        {/* Keeps the line's height until the voices arrive, so the card doesn't jump. */}
        <span className={styles.voice}>{voiceLabel || ' '}</span>
        <p className={styles.line}>{SAMPLE_LINE}</p>
      </div>
    </div>
  );
};

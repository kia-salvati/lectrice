import { PLAYBACK_SPEEDS } from '../../config/constants';
import type { PlaybackSpeed } from '../../config/constants';
import controls from '../../styles/controls.module.css';

interface SpeedPickerProps { value: PlaybackSpeed; onChange: (speed: PlaybackSpeed) => void; }

export const SpeedPicker = ({ value, onChange }: SpeedPickerProps) => (
  <label>
    <span>Speed</span>
    <select
      className={controls.select}
      value={String(value)}
      onChange={(event) => {
        const speed = PLAYBACK_SPEEDS.find((candidate) => String(candidate) === event.target.value);
        if (speed !== undefined) onChange(speed);
      }}
    >
      {PLAYBACK_SPEEDS.map((speed) => <option key={speed} value={String(speed)}>{speed}×</option>)}
    </select>
  </label>
);

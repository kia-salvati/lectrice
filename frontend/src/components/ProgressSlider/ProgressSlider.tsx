import { useState } from 'react';
import styles from './ProgressSlider.module.css';

interface ProgressSliderProps { position: number; max: number; valueText: string; onSeek: (position: number) => void; }

/** Native range. Keeps a draft while dragging and seeks once on release, not once per pixel. */
export const ProgressSlider = ({ position, max, valueText, onSeek }: ProgressSliderProps) => {
  const [draft, setDraft] = useState<number | null>(null);

  const commit = () => {
    if (draft !== null && draft !== position) onSeek(draft);
    setDraft(null);
  };

  return (
    <input
      type="range"
      className={styles.range}
      min={0}
      max={Math.max(0, max)}
      value={draft ?? position}
      aria-label="Position in book"
      aria-valuetext={valueText}
      onChange={(event) => setDraft(Number(event.target.value))}
      onPointerUp={commit}
      onKeyUp={commit}
      onBlur={commit}
    />
  );
};

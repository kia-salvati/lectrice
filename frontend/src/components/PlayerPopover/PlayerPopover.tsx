import type { PlaybackSpeed } from '../../config/constants';
import controls from '../../styles/controls.module.css';
import type { VoiceGroup } from '../../utils/voices';
import { ProgressSlider } from '../ProgressSlider/ProgressSlider';
import { ReadSkippedSwitch } from '../ReadSkippedSwitch/ReadSkippedSwitch';
import { SpeedPicker } from '../SpeedPicker/SpeedPicker';
import { VoicePicker } from '../VoicePicker/VoicePicker';
import styles from './PlayerPopover.module.css';

interface PlayerPopoverProps {
  /** For aria-controls on the "more" button. */
  id: string;
  position: number;
  playableCount: number;
  /** aria-valuetext: 'Sentence 120 of 7,939, page 37' */
  positionText: string;
  /** The <ChapterLine/> above the location line; null before the first chapter or in a book without any. */
  chapterSection: React.ReactNode;
  /** 'Page 37 · PDF page 61 of 429' */
  locationText: string;
  onOpenChapters: () => void;
  timeLeftText: string;
  onSeek: (position: number) => void;
  speed: PlaybackSpeed;
  onSpeedChange: (speed: PlaybackSpeed) => void;
  voiceGroups: VoiceGroup[];
  voiceId: string;
  voicesAvailable: boolean;
  onVoiceChange: (voiceId: string) => void;
  readSkipped: boolean;
  onReadSkippedChange: (readSkipped: boolean) => void;
  exportPanel: React.ReactNode;
  onOpenAnother: () => void;
}

export const PlayerPopover = ({ id, position, playableCount, positionText, chapterSection, locationText, onOpenChapters, timeLeftText, onSeek, speed, onSpeedChange, voiceGroups, voiceId, voicesAvailable, onVoiceChange, readSkipped, onReadSkippedChange, exportPanel, onOpenAnother }: PlayerPopoverProps) => (
  <div id={id} className={styles.popover}>
    <div className={styles.position}>
      <div className={styles.row}>
        <ProgressSlider position={position} max={playableCount - 1} valueText={positionText} onSeek={onSeek} />
        <span className={styles.muted}>{timeLeftText}</span>
      </div>
      {chapterSection}
      <div className={styles.locationRow}>
        <span className={styles.location}>{locationText}</span>
        <button type="button" className={controls.button} onClick={onOpenChapters}>Chapters…</button>
      </div>
    </div>
    <div className={styles.settings}>
      <SpeedPicker value={speed} onChange={onSpeedChange} />
      <VoicePicker groups={voiceGroups} value={voiceId} available={voicesAvailable} onChange={onVoiceChange} />
      <ReadSkippedSwitch checked={readSkipped} onChange={onReadSkippedChange} />
    </div>
    {exportPanel}
    <div className={styles.row}>
      <button type="button" className={controls.button} onClick={onOpenAnother}>Open another PDF…</button>
    </div>
  </div>
);

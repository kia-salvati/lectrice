import { Fragment, useEffect, useRef } from 'react';
import controls from '../../styles/controls.module.css';
import type { PlayerState, StatusPart } from '../../utils/playerStatus';
import { ListIcon, LocateIcon, MoreIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon, WaitIcon } from '../icons/icons';
import styles from './MiniPlayer.module.css';

interface MiniPlayerProps {
  playerState: PlayerState;
  /** Joined with " · "; only a part that shrinks gets the ellipsis. */
  statusParts: readonly StatusPart[];
  canGoBack: boolean;
  canGoForward: boolean;
  /** Auto-scroll was interrupted by the reader; offer a way back. */
  showBackToCurrent: boolean;
  onTogglePlay: () => void;
  onPrevious: () => void;
  onNext: () => void;
  onBackToCurrent: () => void;
  /** The <PlayerPopover/>. The page owns whether it is open, since opening the chapters closes it. */
  popover: React.ReactNode;
  popoverId: string;
  isPopoverOpen: boolean;
  onPopoverOpenChange: (open: boolean) => void;
  /** The chapters button: hidden while the rail shows the chapters already. */
  showChaptersButton: boolean;
  isChaptersOpen: boolean;
  chaptersPanelId: string;
  onToggleChapters: () => void;
}

interface PlayButtonLook { icon: React.ReactNode; label: string; busy: boolean; disabled: boolean; }

const PLAY_BUTTON: Record<PlayerState, PlayButtonLook> = {
  playing: { icon: <PauseIcon />, label: 'Pause', busy: false, disabled: false },
  preparing: { icon: <WaitIcon />, label: 'Preparing audio', busy: true, disabled: false },
  paused: { icon: <PlayIcon />, label: 'Play', busy: false, disabled: false },
  ended: { icon: <PlayIcon />, label: 'Play', busy: false, disabled: false },
  engineFailed: { icon: <PlayIcon />, label: 'Play', busy: false, disabled: true },
};

const STATUS_CLASS: Record<PlayerState, string> = {
  playing: styles.status,
  preparing: `${styles.status} ${styles.prep}`,
  paused: styles.status,
  ended: styles.status,
  engineFailed: styles.status,
};

const pillButton = `${controls.iconButton} ${styles.pillButton}`;

export const MiniPlayer = ({ playerState, statusParts, canGoBack, canGoForward, showBackToCurrent, onTogglePlay, onPrevious, onNext, onBackToCurrent, popover, popoverId, isPopoverOpen, onPopoverOpenChange, showChaptersButton, isChaptersOpen, chaptersPanelId, onToggleChapters }: MiniPlayerProps) => {
  const pillRef = useRef<HTMLDivElement>(null);
  const moreButtonRef = useRef<HTMLButtonElement>(null);
  const play = PLAY_BUTTON[playerState];

  // A disclosure, not a modal: Escape or a click outside closes it; focus is never trapped.
  useEffect(() => {
    if (!isPopoverOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      onPopoverOpenChange(false);
      moreButtonRef.current?.focus();
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !pillRef.current?.contains(event.target)) onPopoverOpenChange(false);
    };
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('pointerdown', handlePointerDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('pointerdown', handlePointerDown);
    };
  }, [isPopoverOpen, onPopoverOpenChange]);

  return (
    <div className={styles.pill} ref={pillRef}>
      <button type="button" className={pillButton} aria-label="Previous sentence" disabled={!canGoBack} onClick={onPrevious}><PrevIcon /></button>
      <button type="button" className={`${controls.iconButton} ${controls.play} ${styles.pillPlay}`} aria-label={play.label} aria-busy={play.busy} disabled={play.disabled} onClick={onTogglePlay}>{play.icon}</button>
      <button type="button" className={pillButton} aria-label="Next sentence" disabled={!canGoForward} onClick={onNext}><NextIcon /></button>
      <span role="status" className={STATUS_CLASS[playerState]}>
        {statusParts.map((part, at) => (
          <Fragment key={at}>
            {at > 0 && <span className={styles.separator} aria-hidden="true">·</span>}
            <span className={part.shrinks ? styles.shrinks : styles.fixed}>{part.text}</span>
          </Fragment>
        ))}
      </span>
      {showBackToCurrent && <button type="button" className={pillButton} aria-label="Scroll to the current sentence" onClick={onBackToCurrent}><LocateIcon /></button>}
      {showChaptersButton && <button type="button" className={pillButton} aria-label="Chapters" aria-expanded={isChaptersOpen} aria-controls={chaptersPanelId} data-chapters-trigger onClick={onToggleChapters}><ListIcon /></button>}
      <button type="button" ref={moreButtonRef} className={pillButton} aria-label="More controls" aria-expanded={isPopoverOpen} aria-controls={popoverId} onClick={() => onPopoverOpenChange(!isPopoverOpen)}><MoreIcon /></button>
      {isPopoverOpen && popover}
    </div>
  );
};

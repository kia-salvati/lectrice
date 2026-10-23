import { useEffect, useRef } from 'react';
import { startedInside } from '../utils/events';

export interface ReaderShortcuts { onTogglePlay: () => void; onNext: () => void; onPrevious: () => void; onToggleChapters: () => void; }

/**
 * Space = play/pause, → / ← = next / previous sentence, C = the chapters panel. Ignored inside form fields, with
 * modifier keys, and inside anything marked `data-own-keys` (the chapters panels drive their own keys).
 */
export const useReaderShortcuts = (shortcuts: ReaderShortcuts): void => {
  const shortcutsRef = useRef(shortcuts);
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  });

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.altKey || event.metaKey) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.matches('input, select, textarea') || startedInside(event, '[data-own-keys]')) return;
      if (event.key === 'c' || event.key === 'C') {
        event.preventDefault();
        shortcutsRef.current.onToggleChapters();
        return;
      }
      if (event.key === ' ') {
        // Space already activates these; and holding the key must not toggle repeatedly.
        if (event.repeat || target?.matches('button, a, summary')) return;
        event.preventDefault();
        shortcutsRef.current.onTogglePlay();
        return;
      }
      if (event.key === 'ArrowRight') {
        event.preventDefault();
        shortcutsRef.current.onNext();
        return;
      }
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        shortcutsRef.current.onPrevious();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
};

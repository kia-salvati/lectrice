import { useEffect, useRef } from 'react';

export interface ReaderShortcuts { onTogglePlay: () => void; onNext: () => void; onPrevious: () => void; }

/** Space = play/pause, → / ← = next / previous sentence. Ignored inside form fields and with modifier keys. */
export const useReaderShortcuts = (shortcuts: ReaderShortcuts): void => {
  const shortcutsRef = useRef(shortcuts);
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  });

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.altKey || event.metaKey) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.matches('input, select, textarea')) return;
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

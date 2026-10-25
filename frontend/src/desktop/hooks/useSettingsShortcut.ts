import { useEffect, useRef } from 'react';
import { SETTINGS_SHORTCUT_KEY } from '../config';

/** Ctrl+, (Cmd+, on a Mac keyboard), the usual preferences shortcut, opens the settings dialog. */
export const useSettingsShortcut = (onOpen: () => void): void => {
  const onOpenRef = useRef(onOpen);
  useEffect(() => {
    onOpenRef.current = onOpen;
  });

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key !== SETTINGS_SHORTCUT_KEY) return;
      event.preventDefault();
      onOpenRef.current();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
};

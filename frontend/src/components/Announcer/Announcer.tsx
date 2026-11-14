import { useCallback, useRef } from 'react';
import styles from './Announcer.module.css';
import { AnnounceContext } from './useAnnounce';
import type { Announce } from './useAnnounce';

/**
 * The app's two live regions. They are on the page from the start and empty, because a screen reader only reads a
 * region whose text changes after it has seen it: a toast that mounts already filled would stay silent.
 */
export const AnnouncerProvider = ({ children }: { children: React.ReactNode }) => {
  const politeRef = useRef<HTMLDivElement>(null);
  const urgentRef = useRef<HTMLDivElement>(null);

  const announce = useCallback<Announce>((text, urgent = false) => {
    const region = (urgent ? urgentRef : politeRef).current;
    if (region === null) return;
    // Emptied first and filled on the next frame, so the same text twice in a row is read twice.
    region.textContent = '';
    requestAnimationFrame(() => {
      region.textContent = text;
    });
  }, []);

  return (
    <AnnounceContext value={announce}>
      <div ref={politeRef} className={styles.srOnly} role="status" aria-live="polite" />
      <div ref={urgentRef} className={styles.srOnly} role="alert" aria-live="assertive" />
      {children}
    </AnnounceContext>
  );
};

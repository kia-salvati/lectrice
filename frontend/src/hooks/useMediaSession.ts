import { useEffect, useRef } from 'react';
import { mediaPlaybackState } from '../utils/mediaSession';
import type { PlaybackStatus } from '../utils/playerStatus';

interface MediaControls { play: () => void; pause: () => void; next: () => void; previous: () => void; }

interface UseMediaSessionArgs extends MediaControls { title: string; status: PlaybackStatus; }

const ACTIONS = ['play', 'pause', 'nexttrack', 'previoustrack'] as const;

const hasMediaSession = (): boolean => 'mediaSession' in navigator;

/** Media keys and the OS media controls (when the browser passes them on) play, pause and step by sentence. */
export const useMediaSession = ({ title, status, play, pause, next, previous }: UseMediaSessionArgs): void => {
  const controlsRef = useRef<MediaControls>({ play, pause, next, previous });
  useEffect(() => {
    controlsRef.current = { play, pause, next, previous };
  });

  useEffect(() => {
    if (!hasMediaSession()) return;
    const session = navigator.mediaSession;
    const handlers: Record<(typeof ACTIONS)[number], () => void> = {
      play: () => controlsRef.current.play(),
      pause: () => controlsRef.current.pause(),
      nexttrack: () => controlsRef.current.next(),
      previoustrack: () => controlsRef.current.previous(),
    };
    for (const action of ACTIONS) session.setActionHandler(action, handlers[action]);
    return () => {
      for (const action of ACTIONS) session.setActionHandler(action, null);
      session.playbackState = 'none';
    };
  }, []);

  useEffect(() => {
    if (!hasMediaSession()) return;
    navigator.mediaSession.metadata = new MediaMetadata({ title });
    return () => {
      navigator.mediaSession.metadata = null;
    };
  }, [title]);

  useEffect(() => {
    if (hasMediaSession()) navigator.mediaSession.playbackState = mediaPlaybackState(status);
  }, [status]);
};

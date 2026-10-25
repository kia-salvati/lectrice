import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../api';
import { ClientError, hasErrorName, isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import { DEFAULT_SPEED, POSITION_SAVE_DELAY_MS, PREFETCH_AHEAD, SECONDS_PER_CHAR } from '../config/constants';
import type { PlaybackSpeed } from '../config/constants';
import type { OpenedDocument } from '../types/document';
import { buildPlaybackOrder, firstPlayableFromBlock, prefetchWindow, secondsLeft, startingSentenceId, stepFrom } from '../utils/playbackOrder';
import type { PlaybackOrder } from '../utils/playbackOrder';
import type { PlaybackStatus } from '../utils/playerStatus';
import { createSentenceAudioBuffer } from './sentenceAudioBuffer';
import type { SentenceAudioBuffer } from './sentenceAudioBuffer';
import { useMediaSession } from './useMediaSession';

/** Which sentence the audio element holds, for which voice buffer and retry round. */
interface LoadedAudio { buffer: SentenceAudioBuffer; sentenceId: number; reloadToken: number; }

interface UsePlaybackArgs {
  document: OpenedDocument;
  voiceId: string;
  /** False while the engine is checking or failed: play is refused and no audio is requested. */
  canPlay: boolean;
  onEngineUnavailable: () => void;
}

export interface Playback {
  /** Always a playable sentence of the current order, or null if nothing is playable. */
  currentSentenceId: number | null;
  /** Index in document.blocks of the block being read (for highlighting code/table boxes and cues). */
  currentBlockIndex: number | null;
  currentPageIndex: number | null;
  /** 0-based position in the playback order, and its length. */
  position: number;
  playableCount: number;
  status: PlaybackStatus;
  speed: PlaybackSpeed;
  /** "Read code & tables aloud". */
  readSkipped: boolean;
  secondsLeft: number;
  /** What is read, in order: for time estimates between two sentences (the chapter's "min left"). */
  order: PlaybackOrder;
  /** Audio for the current sentence could not be made or played. Engine failures go to onEngineUnavailable instead. */
  audioError: AppError | null;
  togglePlay: () => void;
  next: () => void;
  previous: () => void;
  /** Any sentence id, even a non-playable one (it resolves to the next playable sentence). */
  jumpTo: (sentenceId: number) => void;
  jumpToBlock: (blockIndex: number) => void;
  seekToPosition: (position: number) => void;
  setSpeed: (speed: PlaybackSpeed) => void;
  setReadSkipped: (readSkipped: boolean) => void;
  retryAudio: () => void;
  skipFailedSentence: () => void;
}

const playbackStatus = ({ ended, playing, isLoaded }: { ended: boolean; playing: boolean; isLoaded: boolean }): PlaybackStatus => {
  if (ended) return 'ended';
  if (!playing) return 'paused';
  return isLoaded ? 'playing' : 'preparing';
};

export const usePlayback = ({ document, voiceId, canPlay, onEngineUnavailable }: UsePlaybackArgs): Playback => {
  const [readSkipped, setReadSkipped] = useState(false);
  const order = useMemo(() => buildPlaybackOrder(document, readSkipped), [document, readSkipped]);
  // The user's last request, which may be any sentence id. The playable current sentence is derived from it,
  // so flipping readSkipped mid-book needs no extra state: a cue becomes the block's first sentence and back.
  const [requestedSentenceId, setRequestedSentenceId] = useState<number | null>(document.lastSentenceId);
  const currentSentenceId = startingSentenceId(order, requestedSentenceId);
  const [wantsToPlay, setWantsToPlay] = useState(false);
  const [ended, setEnded] = useState(false);
  const [loaded, setLoaded] = useState<LoadedAudio | null>(null);
  const [speed, setSpeed] = useState<PlaybackSpeed>(DEFAULT_SPEED);
  const [audioError, setAudioError] = useState<AppError | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const currentRef = useRef(currentSentenceId);
  const orderRef = useRef(order);
  const speedRef = useRef(speed);
  const engineUnavailableRef = useRef(onEngineUnavailable);

  useEffect(() => {
    currentRef.current = currentSentenceId;
    orderRef.current = order;
    speedRef.current = speed;
    engineUnavailableRef.current = onEngineUnavailable;
  });

  const buffer = useMemo(() => createSentenceAudioBuffer(document.id, voiceId), [document.id, voiceId]);
  useEffect(() => () => buffer.clear(), [buffer]);
  // Declared before the load effect so the first request already carries the right rate and switch.
  useEffect(() => buffer.setContext({ rate: speed, readSkipped }), [buffer, speed, readSkipped]);

  // One audio element for the whole session: it keeps the playback rate and a single set of listeners.
  useEffect(() => {
    const audio = new Audio();
    audio.preload = 'auto';
    audioRef.current = audio;
    const handleEnded = () => {
      const current = currentRef.current;
      const following = current === null ? null : stepFrom(orderRef.current, current, 1);
      if (following === null) {
        setWantsToPlay(false);
        setEnded(true);
        return;
      }
      setRequestedSentenceId(following);
    };
    const handleError = () => {
      if (!audio.getAttribute('src')) return;
      setWantsToPlay(false);
      setAudioError(new ClientError('AudioPlaybackFailed'));
    };
    audio.addEventListener('ended', handleEnded);
    audio.addEventListener('error', handleError);
    return () => {
      audio.removeEventListener('ended', handleEnded);
      audio.removeEventListener('error', handleError);
      audio.pause();
      audio.removeAttribute('src');
      audio.load();
      audioRef.current = null;
    };
  }, []);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.defaultPlaybackRate = speed;
    audio.playbackRate = speed;
  }, [speed]);

  // Load the current sentence into the element and prefetch the next ones. Nothing is requested while the
  // engine is down: health already says so, and every audio request would only fail the same way.
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || currentSentenceId === null || !canPlay) return;
    let cancelled = false;
    audio.pause();
    const wanted = prefetchWindow(order, currentSentenceId, PREFETCH_AHEAD);
    buffer.keepOnly(wanted);
    buffer.get(currentSentenceId).then((url) => {
      if (cancelled) return;
      audio.src = url;
      // Loading a new src resets playbackRate to defaultPlaybackRate, so set both.
      audio.defaultPlaybackRate = speedRef.current;
      audio.playbackRate = speedRef.current;
      setLoaded({ buffer, sentenceId: currentSentenceId, reloadToken });
    }).catch((error: unknown) => {
      if (cancelled || isAbortError(error)) return;
      setWantsToPlay(false);
      if (hasErrorName(error, 'SpeechEngineUnavailable')) {
        engineUnavailableRef.current();
        return;
      }
      console.error('Sentence audio failed', error);
      setAudioError(toAppError(error));
    });
    for (const sentenceId of wanted.slice(1)) {
      buffer.get(sentenceId).catch((error: unknown) => {
        if (!isAbortError(error)) console.warn('Prefetch failed; will retry when needed', error);
      });
    }
    return () => {
      cancelled = true;
    };
  }, [buffer, order, currentSentenceId, reloadToken, canPlay]);

  // Play or pause the element to match intent, once the current sentence is loaded.
  const isLoaded = loaded !== null && loaded.buffer === buffer && loaded.sentenceId === currentSentenceId && loaded.reloadToken === reloadToken;
  // The engine failing (canPlay false) stops playback without extra state.
  const isPlaying = wantsToPlay && canPlay;
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (!isPlaying || !isLoaded) {
      audio.pause();
      return;
    }
    audio.play().catch((error: unknown) => {
      // AbortError: play() was interrupted by pause() or a new src, which is expected when jumping.
      if (isAbortError(error)) return;
      console.error('Audio play() failed', error);
      setWantsToPlay(false);
      if (!(error instanceof DOMException && error.name === 'NotAllowedError')) setAudioError(new ClientError('AudioPlaybackFailed', { cause: error }));
    });
  }, [isPlaying, isLoaded]);

  // Save the position a moment after it stops changing, on pause, and when leaving.
  // Starts at the opening sentence, so just opening a book (or reopening it) doesn't save a position.
  const lastSavedRef = useRef(currentSentenceId);
  const savePosition = useCallback((sentenceId: number) => {
    if (sentenceId === lastSavedRef.current) return;
    lastSavedRef.current = sentenceId;
    api.saveReadingPosition(document.id, sentenceId).catch((error: unknown) => console.warn('Could not save reading position', error));
  }, [document.id]);

  useEffect(() => {
    if (currentSentenceId === null) return;
    const timer = setTimeout(() => savePosition(currentSentenceId), POSITION_SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [currentSentenceId, savePosition]);

  useEffect(() => {
    const flush = () => {
      if (currentRef.current !== null) savePosition(currentRef.current);
    };
    window.addEventListener('pagehide', flush);
    return () => {
      window.removeEventListener('pagehide', flush);
      flush();
    };
  }, [savePosition]);

  const jumpTo = useCallback((sentenceId: number) => {
    setEnded(false);
    setAudioError(null);
    setRequestedSentenceId(sentenceId);
  }, []);

  const jumpToBlock = useCallback((blockIndex: number) => {
    const target = firstPlayableFromBlock(order, blockIndex);
    if (target !== null) jumpTo(target);
  }, [order, jumpTo]);

  const pause = useCallback(() => {
    setWantsToPlay(false);
    if (currentRef.current !== null) savePosition(currentRef.current);
  }, [savePosition]);

  const play = useCallback(() => {
    if (!canPlay || currentSentenceId === null) return;
    setAudioError(null);
    if (ended) {
      setEnded(false);
      setRequestedSentenceId(order.sentenceIds[0] ?? null);
    }
    setWantsToPlay(true);
  }, [canPlay, currentSentenceId, ended, order]);

  const togglePlay = useCallback(() => (isPlaying ? pause() : play()), [isPlaying, pause, play]);

  const step = useCallback((offset: number) => {
    if (currentSentenceId === null) return;
    const target = stepFrom(order, currentSentenceId, offset);
    if (target !== null) jumpTo(target);
  }, [currentSentenceId, order, jumpTo]);

  const next = useCallback(() => step(1), [step]);
  const previous = useCallback(() => step(-1), [step]);

  const seekToPosition = useCallback((position: number) => {
    const target = order.sentenceIds[position];
    if (target !== undefined) jumpTo(target);
  }, [order, jumpTo]);

  const retryAudio = useCallback(() => {
    setAudioError(null);
    setReloadToken((token) => token + 1);
    setWantsToPlay(canPlay);
  }, [canPlay]);

  const skipFailedSentence = useCallback(() => {
    next();
    setWantsToPlay(canPlay);
  }, [next, canPlay]);

  const status = playbackStatus({ ended, playing: isPlaying, isLoaded });
  useMediaSession({ title: document.title, status, play, pause, next, previous });

  const position = currentSentenceId === null ? 0 : order.positionOf.get(currentSentenceId) ?? 0;

  return {
    currentSentenceId,
    currentBlockIndex: currentSentenceId === null ? null : order.blockIndexAt[position] ?? null,
    currentPageIndex: currentSentenceId === null ? null : order.pageIndexOf.get(currentSentenceId) ?? null,
    position,
    playableCount: order.sentenceIds.length,
    status,
    speed,
    readSkipped,
    secondsLeft: currentSentenceId === null ? 0 : secondsLeft(order, currentSentenceId, speed, SECONDS_PER_CHAR),
    order,
    audioError,
    togglePlay,
    next,
    previous,
    jumpTo,
    jumpToBlock,
    seekToPosition,
    setSpeed,
    setReadSkipped,
    retryAudio,
    skipFailedSentence,
  };
};

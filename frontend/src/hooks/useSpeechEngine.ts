import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import { isAbortError } from '../api/errors';
import type { EngineState } from '../utils/playerStatus';

export interface SpeechEngine {
  state: EngineState;
  /** The banner's Retry button. */
  recheck: () => void;
  /** Called by usePlayback and useAudioExport when a request fails with SpeechEngineUnavailable. */
  markUnavailable: () => void;
}

export const useSpeechEngine = (): SpeechEngine => {
  const [state, setState] = useState<EngineState>('checking');
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    api.getHealth(controller.signal).then((health) => setState(health.speechEngine)).catch((cause: unknown) => {
      if (isAbortError(cause)) return;
      console.error('Health check failed', cause);
      setState('unreachable');
    });
    return () => controller.abort();
  }, [attempt]);

  const recheck = useCallback(() => {
    setState('checking');
    setAttempt((count) => count + 1);
  }, []);

  const markUnavailable = useCallback(() => setState('failed'), []);

  return { state, recheck, markUnavailable };
};

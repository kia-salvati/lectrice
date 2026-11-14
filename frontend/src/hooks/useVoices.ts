import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import { isAbortError } from '../api/errors';
import type { Voice } from '../types/voice';

export interface Voices {
  voices: Voice[];
  status: 'loading' | 'ready' | 'failed';
  /** Server default until the user picks one. Null until loaded: nothing plays or exports before then. */
  selectedVoiceId: string | null;
  selectVoice: (voiceId: string) => void;
  /** Asks for the list again after it failed. */
  retry: () => void;
}

export const useVoices = (): Voices => {
  const [voices, setVoices] = useState<Voice[]>([]);
  const [status, setStatus] = useState<Voices['status']>('loading');
  const [selectedVoiceId, setSelectedVoiceId] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    api.listVoices(controller.signal).then((list) => {
      setVoices(list.voices);
      setSelectedVoiceId((current) => current ?? list.defaultVoiceId);
      setStatus('ready');
    }).catch((cause: unknown) => {
      if (isAbortError(cause)) return;
      console.error('Voice list failed', cause);
      setStatus('failed');
    });
    return () => controller.abort();
  }, [attempt]);

  const retry = useCallback(() => {
    setStatus('loading');
    setAttempt((count) => count + 1);
  }, []);

  return { voices, status, selectedVoiceId, selectVoice: setSelectedVoiceId, retry };
};

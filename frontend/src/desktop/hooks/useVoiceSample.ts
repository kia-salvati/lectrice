import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../../api';
import { isAbortError } from '../../api/errors';
import { groupVoices } from '../../utils/voices';
import type { SampleState } from '../components/SampleCard/SampleCard';

export interface VoiceSample {
  /** "Heart · American English"; empty until the voices are known. */
  voiceLabel: string;
  state: SampleState;
  toggle: () => void;
}

/** The default voice reading its sample line, for the "voice is ready" card. Asks for the voices once the engine is ready. */
export const useVoiceSample = (engineReady: boolean): VoiceSample => {
  const [voice, setVoice] = useState<{ id: string; label: string } | null>(null);
  const [state, setState] = useState<SampleState>('idle');
  const audioRef = useRef<{ element: HTMLAudioElement; url: string } | null>(null);

  useEffect(() => {
    if (!engineReady) return;
    const controller = new AbortController();
    api.listVoices(controller.signal).then((list) => {
      const group = groupVoices(list.voices).find((candidate) => candidate.voices.some((member) => member.id === list.defaultVoiceId));
      const chosen = group?.voices.find((member) => member.id === list.defaultVoiceId);
      if (group && chosen) setVoice({ id: chosen.id, label: `${chosen.name} · ${group.label}` });
    }).catch((cause: unknown) => {
      if (!isAbortError(cause)) console.error('Voice list failed', cause);
    });
    return () => controller.abort();
  }, [engineReady]);

  const stop = useCallback(() => {
    const current = audioRef.current;
    if (current === null) return;
    current.element.pause();
    URL.revokeObjectURL(current.url);
    audioRef.current = null;
  }, []);
  useEffect(() => stop, [stop]);

  const toggle = useCallback(() => {
    if (state === 'playing') {
      stop();
      setState('idle');
      return;
    }
    if (voice === null || state === 'loading') return;
    setState('loading');
    api.getVoiceSample(voice.id).then(async (blob) => {
      stop();
      const url = URL.createObjectURL(blob);
      const element = new Audio(url);
      audioRef.current = { element, url };
      element.addEventListener('ended', () => {
        stop();
        setState('idle');
      });
      await element.play();
      setState('playing');
    }).catch((cause: unknown) => {
      console.error('Voice sample failed', cause);
      stop();
      setState('idle');
    });
  }, [state, voice, stop]);

  return { voiceLabel: voice?.label ?? '', state: engineReady ? state : 'engineLoading', toggle };
};

import type { VoiceList } from '../../types/api';
import type { Voice } from '../../types/voice';

const voice = (id: string, name: string): Voice => ({
  id,
  name,
  language: id.startsWith('b') ? 'en-GB' : 'en-US',
  gender: id[1] === 'f' ? 'female' : 'male',
});

/** Kokoro English voices, speaker ids 0–27, in id order. */
export const MOCK_VOICES: VoiceList = {
  defaultVoiceId: 'af_heart',
  voices: [
    voice('af_alloy', 'Alloy'), voice('af_aoede', 'Aoede'), voice('af_bella', 'Bella'), voice('af_heart', 'Heart'),
    voice('af_jessica', 'Jessica'), voice('af_kore', 'Kore'), voice('af_nicole', 'Nicole'), voice('af_nova', 'Nova'),
    voice('af_river', 'River'), voice('af_sarah', 'Sarah'), voice('af_sky', 'Sky'),
    voice('am_adam', 'Adam'), voice('am_echo', 'Echo'), voice('am_eric', 'Eric'), voice('am_fenrir', 'Fenrir'),
    voice('am_liam', 'Liam'), voice('am_michael', 'Michael'), voice('am_onyx', 'Onyx'), voice('am_puck', 'Puck'), voice('am_santa', 'Santa'),
    voice('bf_alice', 'Alice'), voice('bf_emma', 'Emma'), voice('bf_isabella', 'Isabella'), voice('bf_lily', 'Lily'),
    voice('bm_daniel', 'Daniel'), voice('bm_fable', 'Fable'), voice('bm_george', 'George'), voice('bm_lewis', 'Lewis'),
  ],
};

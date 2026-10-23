import type { Voice, VoiceGender, VoiceLanguage } from '../types/voice';

export interface VoiceGroup { label: string; voices: Voice[]; }

const LANGUAGE_LABEL: Record<VoiceLanguage, string> = { 'en-US': 'American English', 'en-GB': 'British English' };
const REGION_SHORT: Record<VoiceLanguage, string> = { 'en-US': 'US', 'en-GB': 'UK' };
const GENDER_LABEL: Record<VoiceGender, string> = { female: 'female', male: 'male' };

/** Groups voices by language, keeping the server's order inside each group and the order languages first appear. */
export const groupVoices = (voices: readonly Voice[]): VoiceGroup[] => {
  const groups = new Map<VoiceLanguage, Voice[]>();
  for (const voice of voices) groups.set(voice.language, [...(groups.get(voice.language) ?? []), voice]);
  return [...groups].map(([language, members]) => ({ label: LANGUAGE_LABEL[language], voices: members }));
};

/** "Heart (US, female)" */
export const formatVoiceOption = (voice: Voice): string => `${voice.name} (${REGION_SHORT[voice.language]}, ${GENDER_LABEL[voice.gender]})`;

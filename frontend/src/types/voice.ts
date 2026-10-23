export type VoiceGender = 'female' | 'male';

export type VoiceLanguage = 'en-US' | 'en-GB';

export interface Voice {
  id: string;
  /** Short display name: "Heart". */
  name: string;
  language: VoiceLanguage;
  gender: VoiceGender;
}

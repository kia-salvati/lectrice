import type { VoiceList } from '../types/api';
import { requestBlob, requestJson } from './client';

export const listVoices = (signal?: AbortSignal): Promise<VoiceList> => requestJson<VoiceList>('/voices', { signal });

export const getVoiceSample = (voiceId: string, signal?: AbortSignal): Promise<Blob> => requestBlob(`/voices/${encodeURIComponent(voiceId)}/sample`, { signal });

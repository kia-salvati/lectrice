import type { VoiceList } from '../types/api';
import { requestJson } from './client';

export const listVoices = (signal?: AbortSignal): Promise<VoiceList> => requestJson<VoiceList>('/voices', { signal });

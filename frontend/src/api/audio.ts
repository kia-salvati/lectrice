import type { SentenceAudioRequest } from './contract';
import { requestBlob } from './client';

export const getSentenceAudio = ({ documentId, sentenceId, voiceId, rate, readSkipped }: SentenceAudioRequest, signal?: AbortSignal): Promise<Blob> => {
  const query = new URLSearchParams({ voice: voiceId, rate: String(rate), readSkipped: String(readSkipped) });
  return requestBlob(`/documents/${encodeURIComponent(documentId)}/sentences/${sentenceId}/audio?${query}`, { signal });
};

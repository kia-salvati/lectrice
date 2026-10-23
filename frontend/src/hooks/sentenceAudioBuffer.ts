import { api } from '../api';
import type { DocumentId } from '../types/document';

interface Entry { controller: AbortController; promise: Promise<string>; url: string | null; }

/** Fetches sentence audio as object URLs, and frees what is no longer wanted. Not React: usePlayback owns one per document + voice. */
export interface SentenceAudioBuffer {
  /** Object URL for the sentence's audio. Repeated calls share one request. */
  get(sentenceId: number): Promise<string>;
  /** Applies to requests made from now on. */
  setContext(next: AudioRequestContext): void;
  /** Aborts and revokes every entry not in `sentenceIds`. */
  keepOnly(sentenceIds: readonly number[]): void;
  clear(): void;
}

/** Sent with each request. Changing it needs no new buffer, because the audio itself doesn't change. */
export interface AudioRequestContext { rate: number; readSkipped: boolean; }

export const createSentenceAudioBuffer = (documentId: DocumentId, voiceId: string): SentenceAudioBuffer => {
  const entries = new Map<number, Entry>();
  let context: AudioRequestContext = { rate: 1, readSkipped: false };

  const release = (entry: Entry) => {
    entry.controller.abort();
    if (entry.url) URL.revokeObjectURL(entry.url);
  };

  const get = (sentenceId: number): Promise<string> => {
    const existing = entries.get(sentenceId);
    if (existing) return existing.promise;
    const controller = new AbortController();
    const entry: Entry = { controller, url: null, promise: Promise.resolve('') };
    entry.promise = api.getSentenceAudio({ documentId, sentenceId, voiceId, ...context }, controller.signal).then((blob) => {
      // Released while the body was arriving: never create a URL nobody will revoke.
      if (controller.signal.aborted) throw new DOMException('Aborted', 'AbortError');
      entry.url = URL.createObjectURL(blob);
      return entry.url;
    });
    // A failed request must not stay cached, so the next get() tries again.
    entry.promise.catch(() => {
      if (entries.get(sentenceId) === entry) entries.delete(sentenceId);
    });
    entries.set(sentenceId, entry);
    return entry.promise;
  };

  const keepOnly = (sentenceIds: readonly number[]) => {
    for (const [sentenceId, entry] of entries) {
      if (sentenceIds.includes(sentenceId)) continue;
      release(entry);
      entries.delete(sentenceId);
    }
  };

  const clear = () => keepOnly([]);

  const setContext = (next: AudioRequestContext) => {
    context = next;
  };

  return { get, setContext, keepOnly, clear };
};

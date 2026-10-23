/** Waits like a real request would, and rejects with AbortError if the signal fires first. */
export const delay = (ms: number, signal?: AbortSignal): Promise<void> => new Promise((resolve, reject) => {
  // Declared before abort() so an already-aborted signal doesn't read it before initialization.
  let timer: ReturnType<typeof setTimeout> | undefined;
  const abort = () => {
    clearTimeout(timer);
    reject(new DOMException('Aborted', 'AbortError'));
  };
  if (signal?.aborted) {
    abort();
    return;
  }
  timer = setTimeout(() => {
    signal?.removeEventListener('abort', abort);
    resolve();
  }, ms);
  signal?.addEventListener('abort', abort, { once: true });
});

export const jitter = (baseMs: number, spreadMs: number): number => baseMs + Math.round(Math.random() * spreadMs);

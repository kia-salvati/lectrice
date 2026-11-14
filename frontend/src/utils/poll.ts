/** A failed look waits twice as long as the one before, up to `maxMs`; a successful one starts again at `baseMs`. */
export const pollDelay = (failures: number, baseMs: number, maxMs: number): number => Math.min(baseMs * 2 ** failures, maxMs);

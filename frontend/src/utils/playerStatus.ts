export type PlaybackStatus = 'paused' | 'preparing' | 'playing' | 'ended';

/** `unreachable` means /health itself failed. */
export type EngineState = 'checking' | 'ready' | 'failed' | 'unreachable';

export type PlayerState = 'engineFailed' | 'preparing' | 'playing' | 'paused' | 'ended';

const ENGINE_BLOCKS_PLAYBACK: Record<EngineState, boolean> = { checking: false, ready: false, failed: true, unreachable: true };

export const playerState = (engine: EngineState, status: PlaybackStatus): PlayerState => (ENGINE_BLOCKS_PLAYBACK[engine] ? 'engineFailed' : status);

const STATUS_TEXT: Record<PlayerState, (pageText: string, speed: number) => string> = {
  engineFailed: () => 'Speech engine unavailable',
  preparing: () => 'Preparing audio…',
  playing: (pageText, speed) => (speed === 1 ? pageText : `${pageText} · ${speed}×`),
  paused: (pageText) => `Paused · ${pageText}`,
  ended: () => 'Finished',
};

/** The pill's short status. `pageText` is "p. 37 of 412". */
export const playerStatusText = (state: PlayerState, pageText: string, speed: number): string => STATUS_TEXT[state](pageText, speed);

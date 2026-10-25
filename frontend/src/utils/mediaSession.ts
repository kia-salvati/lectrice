import type { PlaybackStatus } from './playerStatus';

/** "Preparing" shows as playing in the OS controls: the next sentence is on its way, not stopped. */
export const mediaPlaybackState = (status: PlaybackStatus): 'playing' | 'paused' => (status === 'playing' || status === 'preparing' ? 'playing' : 'paused');

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { webPlatform } from '../src/platform/web.ts';
import { mediaPlaybackState } from '../src/utils/mediaSession.ts';
import type { PlaybackStatus } from '../src/utils/playerStatus.ts';

test('web platform keeps the same-origin API and both export formats, and has no desktop shell', () => {
  assert.equal(webPlatform.apiBaseUrl, '/api');
  assert.deepEqual(webPlatform.exportFormats, ['mp3', 'm4b']);
  assert.equal(webPlatform.desktop, null);
});

test('media session state follows playback', () => {
  const expected: Record<PlaybackStatus, 'playing' | 'paused'> = { playing: 'playing', preparing: 'playing', paused: 'paused', ended: 'paused' };
  for (const [status, state] of Object.entries(expected)) assert.equal(mediaPlaybackState(status as PlaybackStatus), state);
});

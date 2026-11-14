import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseRoute, routeHref } from '../src/config/routes.ts';
import { createSentenceAudioBuffer } from '../src/hooks/sentenceAudioBuffer.ts';
import type { SentenceAudioRequest } from '../src/api/contract.ts';
import { isInProgress, isStatusInProgress } from '../src/utils/exportStatus.ts';
import { pollDelay } from '../src/utils/poll.ts';

test('a route survives the round trip, and anything malformed is the library', () => {
  assert.deepEqual(parseRoute(routeHref({ name: 'reader', documentId: 'a b/ç' })), { name: 'reader', documentId: 'a b/ç' });
  assert.deepEqual(parseRoute('#/'), { name: 'library' });
  assert.deepEqual(parseRoute(''), { name: 'library' });
  assert.deepEqual(parseRoute('#/read/'), { name: 'library' });
  assert.deepEqual(parseRoute('#/elsewhere/x'), { name: 'library' });
  const warn = console.warn;
  console.warn = () => {};
  try {
    assert.deepEqual(parseRoute('#/read/%E0%A4%A'), { name: 'library' });
  } finally {
    console.warn = warn;
  }
});

interface PendingAudio { request: SentenceAudioRequest; signal: AbortSignal | undefined; resolve: (blob: Blob) => void; reject: (cause: unknown) => void; }

const fakeAudioSource = () => {
  const pending: PendingAudio[] = [];
  const fetchAudio = (request: SentenceAudioRequest, signal?: AbortSignal) => new Promise<Blob>((resolve, reject) => {
    pending.push({ request, signal, resolve, reject });
  });
  return { pending, fetchAudio };
};

test('the audio buffer shares one request per sentence and sends the latest context', async () => {
  const source = fakeAudioSource();
  const buffer = createSentenceAudioBuffer(source.fetchAudio, 'doc', 'voice');
  buffer.setContext({ rate: 1.5, readSkipped: true });
  const first = buffer.get(3);
  assert.equal(buffer.get(3), first);
  assert.equal(source.pending.length, 1);
  assert.deepEqual(source.pending[0].request, { documentId: 'doc', sentenceId: 3, voiceId: 'voice', rate: 1.5, readSkipped: true });
  source.pending[0].resolve(new Blob(['x']));
  const url = await first;
  assert.match(url, /^blob:/);
  buffer.clear();
});

test('the audio buffer aborts what is no longer wanted and retries a failed sentence', async () => {
  const source = fakeAudioSource();
  const buffer = createSentenceAudioBuffer(source.fetchAudio, 'doc', 'voice');
  const dropped = buffer.get(1);
  const kept = buffer.get(2);
  buffer.keepOnly([2]);
  assert.equal(source.pending[0].signal?.aborted, true);
  assert.equal(source.pending[1].signal?.aborted, false);
  source.pending[0].resolve(new Blob(['late']));
  await assert.rejects(dropped, (cause: unknown) => cause instanceof DOMException && cause.name === 'AbortError');

  source.pending[1].reject(new Error('engine hiccup'));
  await assert.rejects(kept);
  await Promise.resolve();
  void buffer.get(2).catch(() => {});
  assert.equal(source.pending.length, 3, 'a failed sentence is asked for again');
  buffer.clear();
  assert.equal(source.pending[2].signal?.aborted, true);
});

test('a failed export poll waits longer each time, up to the cap', () => {
  assert.deepEqual([0, 1, 2, 3, 10].map((failures) => pollDelay(failures, 1000, 30_000)), [1000, 2000, 4000, 8000, 30_000]);
});

test('only queued and running exports are polled', () => {
  assert.deepEqual((['queued', 'running', 'paused', 'done', 'failed', 'cancelled'] as const).map(isStatusInProgress), [true, true, false, false, false, false]);
  assert.equal(isInProgress({ status: 'running' } as Parameters<typeof isInProgress>[0]), true);
});

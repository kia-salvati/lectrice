import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatTimeLeft, formatRelativeTime } from '../src/utils/time.ts';
import { buildPlaybackOrder, firstPlayableFromBlock, prefetchWindow, resolvePlayable, startingSentenceId, stepFrom } from '../src/utils/playbackOrder.ts';
import { planPageMarkers } from '../src/utils/pages.ts';
import { checkPdfFile } from '../src/utils/pdfFile.ts';
import { playerState, playerStatusText } from '../src/utils/playerStatus.ts';
import type { PlayerState } from '../src/utils/playerStatus.ts';
import { exportPhase } from '../src/utils/exportPhase.ts';
import { safeFileName } from '../src/utils/fileName.ts';
import { percentRead } from '../src/utils/progress.ts';
import { formatVoiceOption, groupVoices } from '../src/utils/voices.ts';
import type { CleanDocument } from '../src/types/document.ts';
import type { AudioExport, ExportStatus } from '../src/types/api.ts';
import type { Voice } from '../src/types/voice.ts';

// Ids: heading 0; code cue 1 + code lines 2, 3; paragraph 4, 5; caption 6 (no cue); paragraph 7.
const doc: CleanDocument = { id: 'd', title: 't', sourceFileName: 'f.pdf', pageCount: 3, cleanerVersion: 1, pages: [], blocks: [
  { id: 0, kind: 'heading', level: 1, pageIndex: 1, sentences: [{ id: 0, text: 'Hi', pageIndex: 1 }] },
  { id: 1, kind: 'code', spoken: false, pageIndex: 2, text: 'a\nb', cue: { id: 1, text: 'Code example.', pageIndex: 2 }, sentences: [{ id: 2, text: 'a', pageIndex: 2 }, { id: 3, text: 'b', pageIndex: 2 }] },
  { id: 2, kind: 'paragraph', pageIndex: 2, sentences: [{ id: 4, text: 'abc', pageIndex: 2 }, { id: 5, text: 'de', pageIndex: 3 }] },
  { id: 3, kind: 'caption', spoken: false, pageIndex: 3, text: 'Fig.', sentences: [{ id: 6, text: 'Fig.', pageIndex: 3 }] },
  { id: 4, kind: 'paragraph', pageIndex: 3, sentences: [{ id: 7, text: 'End.', pageIndex: 3 }] },
] };

test('time left', () => {
  assert.equal(formatTimeLeft(30), 'under a minute left');
  assert.equal(formatTimeLeft(12 * 60), '12 min left');
  assert.equal(formatTimeLeft(185 * 60), '3 h 05 min left');
  assert.equal(formatRelativeTime('2026-10-20T10:00:00Z', new Date('2026-10-21T11:00:00Z')), 'yesterday');
});

test('readSkipped off: cue instead of code, caption skipped silently', () => {
  const order = buildPlaybackOrder(doc, false);
  assert.deepEqual(order.sentenceIds, [0, 1, 4, 5, 7]);
  assert.deepEqual(order.blockIndexAt, [0, 1, 2, 2, 4]);
  assert.deepEqual(order.charsFrom, [2 + 13 + 3 + 2 + 4, 13 + 3 + 2 + 4, 3 + 2 + 4, 2 + 4, 4, 0]);
  assert.equal(stepFrom(order, 0, 1), 1);
  assert.equal(stepFrom(order, 7, 1), null);
  assert.deepEqual(prefetchWindow(order, 1, 2), [1, 4, 5]);
});

test('readSkipped on: own sentences, never the cue', () => {
  const order = buildPlaybackOrder(doc, true);
  assert.deepEqual(order.sentenceIds, [0, 2, 3, 4, 5, 6, 7]);
});

test('any id resolves onto the current order', () => {
  const off = buildPlaybackOrder(doc, false);
  const on = buildPlaybackOrder(doc, true);
  assert.equal(resolvePlayable(off, 3), 1);   // a code line → the code block's cue
  assert.equal(resolvePlayable(on, 1), 2);    // the cue → the code's first line
  assert.equal(resolvePlayable(off, 6), 7);   // a caption → the next playable block
  assert.equal(resolvePlayable(off, 99), null);
  assert.equal(startingSentenceId(off, null), 0);
  assert.equal(startingSentenceId(off, 99), 0);
  assert.equal(firstPlayableFromBlock(off, 3), 7);
});

test('page markers', () => {
  const plan = planPageMarkers(doc);
  assert.deepEqual([...plan.beforeSentence], [0, 5]);
  assert.deepEqual([...plan.beforeBlock], [1]);
});

test('pdf checks', () => {
  assert.equal(checkPdfFile({ name: 'a.pdf', type: '', size: 10 }, 100), null);
  assert.equal(checkPdfFile({ name: 'a.txt', type: 'text/plain', size: 10 }, 100), 'FileNotPdf');
  assert.equal(checkPdfFile({ name: 'a.pdf', type: 'application/pdf', size: 101 }, 100), 'FileTooLarge');
});

// Ids: note 0; table cue 1 + cells 2, 3.
const tableDoc: CleanDocument = { id: 'd2', title: 't', sourceFileName: 'f.pdf', pageCount: 1, cleanerVersion: 1, pages: [], blocks: [
  { id: 0, kind: 'note', pageIndex: 1, sentences: [{ id: 0, text: 'Note.', pageIndex: 1 }] },
  { id: 1, kind: 'table', spoken: false, pageIndex: 1, text: 'a  b', cue: { id: 1, text: 'Table.', pageIndex: 1 }, sentences: [{ id: 2, text: 'a', pageIndex: 1 }, { id: 3, text: 'b', pageIndex: 1 }] },
] };

test('a note always plays; a table plays its cue when off and its sentences when on', () => {
  assert.deepEqual(buildPlaybackOrder(tableDoc, false).sentenceIds, [0, 1]);
  assert.deepEqual(buildPlaybackOrder(tableDoc, true).sentenceIds, [0, 2, 3]);
  assert.deepEqual([...planPageMarkers(tableDoc).beforeSentence], [0]);
  assert.deepEqual([...planPageMarkers(tableDoc).beforeBlock], []);
});

// Ids: paragraph 0, silent 1, 2 (spoken as "Go."); silent code line 4 (cue 3) + line 5; silent paragraph 6.
const silentDoc: CleanDocument = { id: 'd3', title: 't', sourceFileName: 'f.pdf', pageCount: 1, cleanerVersion: 1, pages: [], blocks: [
  { id: 0, kind: 'paragraph', pageIndex: 1, sentences: [{ id: 0, text: 'One.', pageIndex: 1 }, { id: 1, text: 'https://x.io', speechText: '', pageIndex: 1 }, { id: 2, text: 'Go [3].', speechText: 'Go.', pageIndex: 1 }] },
  { id: 1, kind: 'code', spoken: false, pageIndex: 1, text: 'x\ny', cue: { id: 3, text: 'Code example.', pageIndex: 1 }, sentences: [{ id: 4, text: 'x', speechText: '', pageIndex: 1 }, { id: 5, text: 'y', pageIndex: 1 }] },
  { id: 2, kind: 'paragraph', pageIndex: 1, sentences: [{ id: 6, text: 'Smith, J. (2020).', speechText: '', pageIndex: 1 }] },
] };

test('silent sentences are skipped by playback but still resolve to the next playable one', () => {
  const off = buildPlaybackOrder(silentDoc, false);
  const on = buildPlaybackOrder(silentDoc, true);
  assert.deepEqual(off.sentenceIds, [0, 2, 3]);
  assert.deepEqual(on.sentenceIds, [0, 2, 5]);
  assert.deepEqual(off.charsFrom, [4 + 3 + 13, 3 + 13, 13, 0]);   // speechText sizes the time left
  assert.equal(resolvePlayable(off, 1), 2);   // mid-paragraph: the next sentence, not the paragraph's start
  assert.equal(resolvePlayable(on, 4), 5);
  assert.equal(resolvePlayable(off, 4), 3);   // switch off: a code line still maps to the cue
  assert.equal(resolvePlayable(off, 6), null);
  assert.equal(stepFrom(off, 0, 1), 2);
});

test('player state and status text', () => {
  assert.equal(playerState('failed', 'playing'), 'engineFailed');
  assert.equal(playerState('unreachable', 'paused'), 'engineFailed');
  assert.equal(playerState('checking', 'paused'), 'paused');
  assert.equal(playerState('ready', 'preparing'), 'preparing');
  const expected: Record<PlayerState, string> = {
    engineFailed: 'Speech engine unavailable',
    preparing: 'Preparing audio…',
    playing: 'p. 37 of 412',
    paused: 'Paused · p. 37 of 412',
    ended: 'Finished',
  };
  for (const [state, text] of Object.entries(expected)) assert.equal(playerStatusText(state as PlayerState, 'p. 37 of 412', 1), text);
  assert.equal(playerStatusText('playing', 'p. 37 of 412', 1.25), 'p. 37 of 412 · 1.25×');
});

test('export phase', () => {
  const job = (status: ExportStatus): AudioExport => ({ id: 'e', documentId: 'd', voiceId: 'af_heart', format: 'mp3', status, sentencesDone: 0, sentencesTotal: 10, estimatedSecondsLeft: null });
  assert.equal(exportPhase(null, false), 'idle');
  assert.equal(exportPhase(null, true), 'starting');
  assert.equal(exportPhase(job('running'), true), 'starting');
  const statuses: ExportStatus[] = ['queued', 'running', 'done', 'failed', 'cancelled'];
  for (const status of statuses) assert.equal(exportPhase(job(status), false), status);
});

test('safe file name', () => {
  assert.equal(safeFileName('A/B: C? (Heart).mp3'), 'A-B- C- (Heart).mp3');
  assert.equal(safeFileName('  many   spaces  '), 'many spaces');
  assert.equal(safeFileName('x'.repeat(200)).length, 120);
});

test('percent read', () => {
  assert.equal(percentRead(null, 70), null);
  assert.equal(percentRead(0, 70), 1);
  assert.equal(percentRead(69, 70), 100);
  assert.equal(percentRead(23, 70), 34);
});

test('voices: option text and grouping by first appearance', () => {
  const heart: Voice = { id: 'af_heart', name: 'Heart', language: 'en-US', gender: 'female' };
  const adam: Voice = { id: 'am_adam', name: 'Adam', language: 'en-US', gender: 'male' };
  const emma: Voice = { id: 'bf_emma', name: 'Emma', language: 'en-GB', gender: 'female' };
  assert.equal(formatVoiceOption(heart), 'Heart (US, female)');
  assert.equal(formatVoiceOption(emma), 'Emma (UK, female)');
  assert.deepEqual(groupVoices([emma, heart, adam]), [
    { label: 'British English', voices: [emma] },
    { label: 'American English', voices: [heart, adam] },
  ]);
});

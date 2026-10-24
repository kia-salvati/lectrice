import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatTimeLeft, formatRelativeTime } from '../src/utils/time.ts';
import { buildPlaybackOrder, firstPlayableFromBlock, prefetchWindow, resolvePlayable, secondsBetween, startingSentenceId, stepFrom } from '../src/utils/playbackOrder.ts';
import { planPageMarkers } from '../src/utils/pages.ts';
import { checkPdfFile } from '../src/utils/pdfFile.ts';
import { playerState, playerStatusParts } from '../src/utils/playerStatus.ts';
import type { PlayerState, StatusLocation, StatusPart } from '../src/utils/playerStatus.ts';
import { exportPhase } from '../src/utils/exportPhase.ts';
import { safeFileName } from '../src/utils/fileName.ts';
import { percentRead } from '../src/utils/progress.ts';
import { formatVoiceOption, groupVoices } from '../src/utils/voices.ts';
import { buildTocTree, chapterLabel, childrenText, currentEntryIndex, entryEndSentenceId, entryLength, entryProgress, filterTocTree, flattenTocTree, isChapterNode, isPartNode, nodeView, percentThrough, tocLocation, tocPath } from '../src/utils/toc.ts';
import type { CleanDocument, TocEntry, TocLevel } from '../src/types/document.ts';
import type { AudioExport, ExportStatus } from '../src/types/api.ts';
import type { Voice } from '../src/types/voice.ts';

// Ids: heading 0; code cue 1 + code lines 2, 3; paragraph 4, 5; caption 6 (no cue); paragraph 7.
const doc: CleanDocument = { id: 'd', title: 't', sourceFileName: 'f.pdf', pageCount: 3, cleanerVersion: 1, pages: [], toc: [], blocks: [
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
const tableDoc: CleanDocument = { id: 'd2', title: 't', sourceFileName: 'f.pdf', pageCount: 1, cleanerVersion: 1, pages: [], toc: [], blocks: [
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
const silentDoc: CleanDocument = { id: 'd3', title: 't', sourceFileName: 'f.pdf', pageCount: 1, cleanerVersion: 1, pages: [], toc: [], blocks: [
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

test('player state and status parts', () => {
  assert.equal(playerState('failed', 'playing'), 'engineFailed');
  assert.equal(playerState('unreachable', 'paused'), 'engineFailed');
  assert.equal(playerState('checking', 'paused'), 'paused');
  assert.equal(playerState('ready', 'preparing'), 'preparing');
  const text = (parts: StatusPart[]) => parts.map((part) => part.text).join(' · ');
  const noChapter: StatusLocation = { pageText: 'p. 37', lastPageText: '412', chapter: null };
  const expected: Record<PlayerState, string> = {
    engineFailed: 'Speech engine unavailable',
    preparing: 'Preparing audio…',
    playing: 'p. 37 of 412',
    paused: 'Paused · p. 37 of 412',
    ended: 'Finished',
  };
  for (const [state, line] of Object.entries(expected)) assert.equal(text(playerStatusParts(state as PlayerState, noChapter, 1)), line);
  assert.equal(text(playerStatusParts('playing', noChapter, 1.25)), 'p. 37 of 412 · 1.25×');
  // With a chapter the "of 412" goes and only the name may shrink.
  const inChapter: StatusLocation = { pageText: 'p. 213', lastPageText: '412', chapter: { number: '7', name: 'Long Documents' } };
  assert.deepEqual(playerStatusParts('playing', inChapter, 1.5), [
    { text: 'Ch. 7', shrinks: false }, { text: 'Long Documents', shrinks: true }, { text: 'p. 213', shrinks: false }, { text: '1.5×', shrinks: false },
  ]);
  assert.equal(text(playerStatusParts('paused', inChapter, 1)), 'Paused · Ch. 7 · Long Documents · p. 213');
  const unnumbered: StatusLocation = { ...inChapter, chapter: { number: null, name: 'Preface' } };
  assert.deepEqual(playerStatusParts('playing', unnumbered, 1), [{ text: 'Preface', shrinks: true }, { text: 'p. 213', shrinks: false }]);
});

test('export phase', () => {
  const job = (status: ExportStatus): AudioExport => ({
    id: 'e', documentId: 'd', voiceId: 'af_heart', format: 'mp3', speedPercent: 100, engineId: 'x', range: null, rangeLabel: null, title: 't', status,
    pausedBy: null, stage: 'parts', error: null, sentencesDone: 0, sentencesTotal: 10, partsDone: 0, partsTotal: 1, estimatedSecondsLeft: null,
    fileBytes: null, location: null, waitingForEngine: false,
  });
  assert.equal(exportPhase(null, false), 'idle');
  assert.equal(exportPhase(null, true), 'starting');
  assert.equal(exportPhase(job('running'), true), 'starting');
  const statuses: ExportStatus[] = ['queued', 'running', 'done', 'failed', 'cancelled'];
  for (const status of statuses) assert.equal(exportPhase(job(status), false), status);
  assert.equal(exportPhase(job('paused'), false), 'failed', 'a paused export shows as stopped, and Try again continues it');
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

test('seconds between two sentences, resolving silent and skipped ids', () => {
  const order = buildPlaybackOrder(doc, false); // ids 0,1,4,5,7 with 2,13,3,2,4 characters
  assert.equal(secondsBetween(order, 0, 4, 1, 1), 15);
  assert.equal(secondsBetween(order, 4, null, 2, 1), 4.5);
  assert.equal(secondsBetween(order, 2, 6, 1, 1), 18); // code line 2 → its block's cue (1); caption 6 → the next playable, 7
  assert.equal(secondsBetween(order, 7, 0, 1, 1), 0);
});

// A parts › chapters › sections book (Clean Architecture's shape) over sentences 0–99.
const entry = (id: number, title: string, level: TocLevel, sentenceId: number): TocEntry => ({ id, title, level, sentenceId, pageIndex: sentenceId + 1 });
const partsToc: TocEntry[] = [
  entry(0, 'Preface', 1, 2),
  entry(1, 'PART I Introduction', 1, 10),
  entry(2, 'Chapter 1 What Is Design and Architecture?', 2, 12),
  entry(3, 'The Goal?', 3, 15),
  entry(4, 'Conclusion', 3, 20),
  entry(5, 'Chapter 2 A Tale of Two Values', 2, 30),
  entry(6, 'PART II Starting with the Bricks', 1, 50),
  entry(7, 'Chapter 4 Structured Programming', 2, 52),
  entry(8, 'Proof', 3, 60),
];
const SENTENCES = 100;

test('toc tree: three levels and a flat list', () => {
  const roots = buildTocTree(partsToc);
  assert.deepEqual(roots.map((node) => node.entry.id), [0, 1, 6]);
  assert.deepEqual(roots[1].children.map((node) => node.entry.id), [2, 5]);
  assert.deepEqual(roots[1].children[0].children.map((node) => node.entry.id), [3, 4]);
  assert.equal(roots[1].children[0].children[1].parentIndex, 2);
  assert.deepEqual(flattenTocTree(roots).map((node) => node.index), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
  assert.ok(isPartNode(roots[1]));
  assert.ok(!isChapterNode(roots[1]));
  assert.ok(isChapterNode(roots[0])); // a top-level entry that is not a part
  assert.ok(isChapterNode(roots[1].children[0]));
  assert.ok(!isChapterNode(roots[1].children[0].children[0]));
  assert.equal(childrenText(roots[1]), '2 chapters');
  assert.equal(childrenText(roots[1].children[0]), '2 sections');
  assert.equal(childrenText(roots[2]), '1 chapter');
  const flatToc = [entry(0, 'Big Endian vs Little Endian', 1, 0), entry(1, 'How can Cache Systems go wrong', 1, 40)];
  const flat = buildTocTree(flatToc);
  assert.equal(flat.length, 2);
  assert.ok(flat.every((node) => node.children.length === 0 && node.parentIndex === null && isChapterNode(node)));
});

test('current entry, path and end ids', () => {
  assert.equal(currentEntryIndex(partsToc, null), null);
  assert.equal(currentEntryIndex(partsToc, 1), null); // before the first entry (cover)
  assert.equal(currentEntryIndex(partsToc, 2), 0); // an entry's first sentence
  assert.equal(currentEntryIndex(partsToc, 17), 3); // mid-part
  assert.equal(currentEntryIndex(partsToc, 99), 8); // after the last
  assert.deepEqual(tocPath(partsToc, 8), [6, 7, 8]);
  assert.deepEqual(tocPath(partsToc, 5), [1, 5]);
  assert.deepEqual(tocPath(partsToc, 0), [0]);
  assert.equal(entryEndSentenceId(partsToc, 0), 10);
  assert.equal(entryEndSentenceId(partsToc, 2), 30); // chapter 1 ends where chapter 2 starts, not at its own sections
  assert.equal(entryEndSentenceId(partsToc, 5), 50); // the last chapter of a part ends with the part
  assert.equal(entryEndSentenceId(partsToc, 8), null);
  assert.equal(entryLength(partsToc, 1, SENTENCES), 40);
  assert.equal(entryLength(partsToc, 8, SENTENCES), 40);
});

test('current chapter: the nearest chapter-like title, else the top of the path', () => {
  const titles = (toc: TocEntry[], sentenceId: number) => {
    const found = tocLocation(toc, sentenceId);
    return found && [toc[found.chapterIndex].title, found.sectionIndex === null ? null : toc[found.sectionIndex].title];
  };
  assert.equal(titles(partsToc, 0), null);
  assert.deepEqual(tocLocation(partsToc, 65), { path: [6, 7, 8], chapterIndex: 7, sectionIndex: 8 });
  assert.deepEqual(titles(partsToc, 65), ['Chapter 4 Structured Programming', 'Proof']);
  assert.deepEqual(titles(partsToc, 11), ['PART I Introduction', null]);
  assert.deepEqual(titles(partsToc, 5), ['Preface', null]);
  const k8s = [entry(0, '2: Kubernetes principles of operation', 1, 0), entry(1, 'Pods', 2, 8), entry(2, '3: Getting Kubernetes', 1, 20)];
  assert.deepEqual(titles(k8s, 9), ['2: Kubernetes principles of operation', 'Pods']);
  const dds = [entry(0, 'Chapter 1. Introduction', 1, 0), entry(1, 'Part I. Single-Node Patterns', 1, 20), entry(2, 'Motivations', 2, 22), entry(3, 'Chapter 2. The Sidecar Pattern', 2, 30)];
  assert.deepEqual(titles(dds, 25), ['Part I. Single-Node Patterns', 'Motivations']);
  assert.deepEqual(titles(dds, 31), ['Chapter 2. The Sidecar Pattern', null]);
  const posts = [entry(0, 'Big Endian vs Little Endian', 1, 0), entry(1, 'How can Cache Systems go wrong', 1, 40)];
  assert.deepEqual(titles(posts, 45), ['How can Cache Systems go wrong', null]);
  const longBook = [entry(0, '7. Long Documents', 1, 0), entry(1, '7.3 Tables of contents that tell the truth', 2, 10), entry(2, 'Why it matters', 3, 12)];
  assert.deepEqual(titles(longBook, 13), ['7. Long Documents', '7.3 Tables of contents that tell the truth']);
});

test('chapter label: every title style, and two that only start with a number', () => {
  assert.deepEqual(chapterLabel('Chapter 4 Structured Programming'), { number: '4', name: 'Structured Programming' });
  assert.deepEqual(chapterLabel('Chapter 2. The Sidecar Pattern'), { number: '2', name: 'The Sidecar Pattern' });
  assert.deepEqual(chapterLabel('CHAPTER 12: Deployments'), { number: '12', name: 'Deployments' });
  assert.deepEqual(chapterLabel('Ch. IV Proof'), { number: 'IV', name: 'Proof' });
  assert.deepEqual(chapterLabel('4: Working with Pods'), { number: '4', name: 'Working with Pods' });
  assert.deepEqual(chapterLabel('1. Introduction'), { number: '1', name: 'Introduction' });
  assert.deepEqual(chapterLabel('7. Long Documents'), { number: '7', name: 'Long Documents' });
  for (const title of ['Preface', 'PART II Starting with the Bricks', 'Appendix A Architecture Archaeology', '7.3 Tables of contents', '20 Popular Open Source Projects', '10 years ago, Amazon moved']) {
    assert.deepEqual(chapterLabel(title), { number: null, name: title }, title);
  }
});

test('entry progress and percent through', () => {
  assert.equal(entryProgress(partsToc, 2, null), 'unread');
  assert.equal(entryProgress(partsToc, 2, 11), 'unread');
  assert.equal(entryProgress(partsToc, 2, 12), 'current');
  assert.equal(entryProgress(partsToc, 2, 29), 'current');
  assert.equal(entryProgress(partsToc, 2, 30), 'read');
  assert.equal(entryProgress(partsToc, 1, 30), 'current'); // the part stays current through chapter 2
  assert.equal(entryProgress(partsToc, 8, 99), 'current'); // the last entry never reads as finished
  assert.equal(percentThrough(partsToc, 2, 12, SENTENCES), 0);
  assert.equal(percentThrough(partsToc, 2, 21, SENTENCES), 50);
  assert.equal(percentThrough(partsToc, 8, 80, SENTENCES), 50); // open end: measured to the book's end
  assert.equal(percentThrough(partsToc, 2, 40, SENTENCES), 100);
});

test('node view: what a row shows', () => {
  const nodes = flattenTocTree(buildTocTree(partsToc));
  const at65 = tocLocation(partsToc, 65);
  const view = (index: number) => nodeView(partsToc, nodes[index], at65, 65, SENTENCES);
  assert.deepEqual(view(7), { progress: 'current', isHere: false, isPart: false, percent: 27, fillPercent: 27 });
  assert.deepEqual(view(8), { progress: 'current', isHere: true, isPart: false, percent: null, fillPercent: null });
  assert.deepEqual(view(6), { progress: 'current', isHere: false, isPart: true, percent: null, fillPercent: null });
  assert.deepEqual(view(2), { progress: 'read', isHere: false, isPart: false, percent: null, fillPercent: 100 });
  assert.deepEqual(nodeView(partsToc, nodes[7], null, null, SENTENCES), { progress: 'unread', isHere: false, isPart: false, percent: null, fillPercent: 0 });
});

test('filter keeps matches at any level with their ancestors', () => {
  const roots = buildTocTree(partsToc);
  assert.equal(filterTocTree(roots, '  ').length, 3);
  const proof = filterTocTree(roots, 'PROOF');
  assert.deepEqual(flattenTocTree(proof).map((node) => node.entry.id), [6, 7, 8]);
  const chapters = filterTocTree(roots, 'chapter');
  assert.deepEqual(flattenTocTree(chapters).map((node) => node.entry.id), [1, 2, 3, 4, 5, 6, 7, 8]); // a match keeps all its children
  assert.deepEqual(filterTocTree(roots, 'nothing like this'), []);
  assert.equal(roots[1].children.length, 2, 'the original tree is untouched');
});

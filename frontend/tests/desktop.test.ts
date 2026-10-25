import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createMockDesktopCommands } from '../src/desktop/commands/mock.ts';
import { hasDesktopErrorName } from '../src/desktop/commands/errors.ts';
import type { ModelStatus, ModelTransfer } from '../src/desktop/commands/types.ts';
import { toDesktopError, toExportAvailability, toModelStatus } from '../src/desktop/commands/wire.ts';
import type { ModelStatusDto } from '../src/desktop/commands/wire.ts';
import { desktopExportPhase } from '../src/desktop/utils/exportPhase.ts';
import { afterRevealed, afterRevealFailed, savedPathOfEntry, saveStateOfEntry } from '../src/desktop/utils/exportSaves.ts';
import { newlyEnded, toastKindOf } from '../src/desktop/utils/exportToasts.ts';
import { folderNameOf, formatApproxTimeLeft, formatDuration, formatMb, formatProgressMb, formatRetryIn, formatSpace, m4bSeconds, percentOf, versionLine } from '../src/desktop/utils/format.ts';
import { installNeedsBytes, installPhase, smallerThatFits } from '../src/desktop/utils/modelSetup.ts';
import { voiceStatusText } from '../src/desktop/utils/voiceStatus.ts';
import type { AudioExport } from '../src/types/api.ts';

const MB = 1_000_000;
const FP32 = 'kokoro-en-v1_0-fp32';
const INT8 = 'kokoro-en-v1_0-int8';

const untilStatus = (commands: ReturnType<typeof createMockDesktopCommands>, matches: (status: ModelStatus) => boolean): Promise<ModelStatus> => new Promise((resolve) => {
  const stop = commands.onModelStatus((status) => {
    if (!matches(status)) return;
    stop();
    resolve(status);
  });
});

test('formats sizes, progress and time left the way the screens show them', () => {
  assert.equal(formatMb(349.9 * MB), '350 MB');
  assert.equal(formatSpace(23.4 * 1_000 * MB), '23.4 GB');
  assert.equal(formatSpace(610 * MB), '610 MB');
  assert.equal(formatProgressMb(142.7 * MB, 350 * MB), '142 MB of 350 MB');
  assert.equal(percentOf(142 * MB, 350 * MB), 41);
  assert.equal(percentOf(2914, 6940), 42);
  assert.equal(formatApproxTimeLeft(43), 'under a minute left');
  assert.equal(formatApproxTimeLeft(99), 'about 2 min left');
  assert.equal(formatApproxTimeLeft(3900), 'about 1 h 5 min left');
  assert.equal(formatApproxTimeLeft(7200), 'about 2 h left');
  assert.equal(formatRetryIn(30), '30 s');
  assert.equal(formatRetryIn(120), '2 min');
  assert.equal(folderNameOf('/home/sam/Music/Book.m4b'), 'Music');
  assert.equal(folderNameOf('C:\\Users\\Sam\\Music\\Book.m4b'), 'Music');
  assert.equal(versionLine({ appVersion: '0.1.0', coreVersion: '0.3.0', os: 'windows', arch: 'x64' }), 'Version 0.1.0 · core and UI 0.3.0 · Windows x64');
});

test('the mock shell downloads a bundle, pauses, resumes and ends with a ready engine', async () => {
  const commands = createMockDesktopCommands({ timeScale: 0.01 });
  const initial = await commands.modelStatus();
  assert.equal(installPhase(initial), 'choosing');
  assert.equal(installNeedsBytes(initial.bundles[0]), 934 * MB);

  const downloading = untilStatus(commands, (status) => status.transfer.kind === 'downloading');
  await commands.modelDownloadStart(FP32);
  assert.equal((await commands.modelStatus()).transfer.kind, 'starting');
  const progress = (await downloading).transfer as Extract<ModelTransfer, { kind: 'downloading' }>;
  assert.equal(progress.totalBytes, 350 * MB);

  await commands.modelDownloadPause();
  const paused = (await commands.modelStatus()).transfer;
  assert.equal(paused.kind, 'paused');

  const ready = untilStatus(commands, (status) => status.engine === 'ready');
  await commands.modelDownloadStart(FP32);
  const final = await ready;
  assert.equal(final.transfer.kind, 'none');
  assert.deepEqual(final.bundles.map((bundle) => [bundle.installed, bundle.active]), [[true, true], [false, false]]);
  assert.equal(installPhase(final), 'ready');
});

test('the mock shell refuses a download that does not fit and offers the smaller bundle', async () => {
  const commands = createMockDesktopCommands({ freeBytes: 610 * MB, timeScale: 0.01 });
  await commands.modelDownloadStart(FP32);
  const status = await commands.modelStatus();
  assert.deepEqual(status.transfer, { kind: 'failed', bundleId: FP32, source: 'download', error: 'StorageFull' });
  assert.equal(smallerThatFits(status, status.bundles[0])?.id, INT8);
  assert.equal(smallerThatFits({ ...status, freeBytes: 300 * MB }, status.bundles[0]), null);
});

test('the mock shell switches, refuses to delete a bundle an export uses, and saves an export', async () => {
  const commands = createMockDesktopCommands({ bundleUsedByExport: INT8, timeScale: 0.01 });
  await commands.modelImport('archive');
  const imported = await untilStatus(commands, (status) => status.engine === 'ready');
  assert.equal(imported.bundles.find((bundle) => bundle.id === INT8)?.active, true);

  const refused = await commands.modelDelete(INT8);
  assert.equal(refused.deleted, false);
  assert.equal(refused.blockingExports.length, 1);
  assert.deepEqual(await commands.exportSaveAs('export-1'), { outcome: 'saved', path: '/home/sam/Music/Audiobook.m4b' });
});

test('the mock shell fails an import, a save and a reveal the way the real shell reports them', async () => {
  const commands = createMockDesktopCommands({ importError: 'ModelChecksumMismatch', saveAsError: 'StorageFull', revealError: 'ExportNotFound', timeScale: 0.01 });
  const failed = untilStatus(commands, (status) => status.transfer.kind === 'failed');
  assert.equal(await commands.modelImport('folder'), true);
  assert.deepEqual((await failed).transfer, { kind: 'failed', bundleId: null, source: 'import', error: 'ModelChecksumMismatch' });

  await assert.rejects(commands.exportSaveAs('export-1'), (cause: unknown) => hasDesktopErrorName(cause, 'StorageFull'));
  await assert.rejects(commands.exportReveal('export-1'), (cause: unknown) => hasDesktopErrorName(cause, 'ExportNotFound'));
});

const bundleDto = (patch: Partial<ModelStatusDto['bundles'][number]> = {}): ModelStatusDto['bundles'][number] => ({
  id: FP32, variant: 'fp32', name: 'Full quality', downloadBytes: 350 * MB, installedBytes: 384 * MB, installed: false, active: false, usedByExports: [], ...patch,
});
const statusDto = (download: ModelStatusDto['download'], patch: Partial<ModelStatusDto> = {}): ModelStatusDto => ({
  bundles: [bundleDto()], engine: { state: 'missing' }, download, freeBytes: 23_400 * MB, setupDismissed: false, ...patch,
});
const progressDto = { bundleId: FP32, bytesDone: 142 * MB, bytesTotal: 350 * MB } as const;

test("the shell's model status maps onto the screens' states", () => {
  const mapped = toModelStatus(statusDto({ state: 'idle' }, { setupDismissed: true }));
  assert.deepEqual(mapped.bundles[0], { id: FP32, variant: 'fp32', displayName: 'Full quality', downloadBytes: 350 * MB, installedBytes: 384 * MB, installed: false, active: false, usedByExports: [] });
  assert.equal(mapped.setupDismissed, true);
  assert.equal(mapped.transfer.kind, 'none');

  const transfer = (download: ModelStatusDto['download']) => toModelStatus(statusDto(download)).transfer;
  assert.deepEqual(transfer({ state: 'downloading', ...progressDto, bytesPerSecond: 2_100_000, secondsLeft: 99, retrying: false }), { kind: 'downloading', bytesPerSecond: 2_100_000, bundleId: FP32, doneBytes: 142 * MB, totalBytes: 350 * MB });
  assert.equal(transfer({ state: 'downloading', ...progressDto, bytesPerSecond: null, secondsLeft: null, retrying: true }).kind, 'retrying');
  assert.deepEqual(transfer({ state: 'checking', bundleId: null, source: 'import' }), { kind: 'verifying', bundleId: null, source: 'import', stage: 'checking' });
  assert.deepEqual(transfer({ state: 'unpacking', bundleId: FP32, source: 'download' }), { kind: 'verifying', bundleId: FP32, source: 'download', stage: 'unpacking' });
  assert.deepEqual(transfer({ state: 'failed', bundleId: null, source: 'import', error: { name: 'ModelChecksumMismatch', code: 4003, message: '' } }), { kind: 'failed', bundleId: null, source: 'import', error: 'ModelChecksumMismatch' });
  assert.equal((transfer({ state: 'failed', bundleId: FP32, source: 'download', error: { name: 'StorageFailed', code: 9001, message: '' } }) as { error: string }).error, 'ModelDownloadFailed', 'no line of its own');

  const noSpace = toModelStatus(statusDto({ state: 'noSpace', bundleId: FP32, source: 'download', neededBytes: 934 * MB, freeBytes: 610 * MB }, { freeBytes: null }));
  assert.deepEqual(noSpace.transfer, { kind: 'failed', bundleId: FP32, source: 'download', error: 'StorageFull' });
  assert.equal(noSpace.freeBytes, 610 * MB, 'the refusal says how much is free');
});

test('export availability names the fix for each system', () => {
  assert.deepEqual(toExportAvailability({ available: true, problem: null }, 'linux'), { available: true });
  assert.deepEqual(toExportAvailability({ available: false, problem: 'notFound' }, 'linux'), { available: false, reason: 'ffmpegMissing' });
  assert.deepEqual(toExportAvailability({ available: false, problem: 'missingComponents' }, 'linux'), { available: false, reason: 'ffmpegCannotMakeM4b' });
  assert.deepEqual(toExportAvailability({ available: false, problem: 'notRunnable' }, 'windows'), { available: false, reason: 'bundledFfmpegMissing' });
});

test("a command's rejection becomes a DesktopError with the registry name", () => {
  const known = toDesktopError({ error: { name: 'ExportNotFound', code: 3001, message: 'Export not found.' } });
  assert.ok(hasDesktopErrorName(known, 'ExportNotFound'));
  assert.equal(known.code, 3001);
  const original = console.error;
  console.error = () => {};
  try {
    assert.ok(hasDesktopErrorName(toDesktopError('command export_list not allowed'), 'Internal'));
  } finally {
    console.error = original;
  }
});

test('the pill says why there is no voice', () => {
  const status = (download: ModelStatusDto['download'], installed = false) => toModelStatus(statusDto(download, { bundles: [bundleDto({ installed })] }));
  assert.equal(voiceStatusText(status({ state: 'idle' })), 'No voice installed');
  assert.equal(voiceStatusText(status({ state: 'downloading', ...progressDto, bytesPerSecond: null, secondsLeft: null, retrying: false })), 'Getting the voice · 41 %');
  assert.equal(voiceStatusText(status({ state: 'starting', ...progressDto })), 'Getting the voice');
  assert.equal(voiceStatusText(status({ state: 'paused', ...progressDto })), 'No voice installed');
  assert.equal(voiceStatusText(status({ state: 'idle' }, true)), null);
});

test('a toast is due once per export that finishes or stops while the app watches', () => {
  const first = [job({ id: 'old', status: 'done' }), job({ id: 'a', status: 'running' }), job({ id: 'b', status: 'running' }), job({ id: 'c', status: 'queued' })];
  assert.deepEqual(newlyEnded(null, first), [], 'nothing at the first look');
  const seen = new Map(first.map((item) => [item.id, item.status]));
  const later = [
    job({ id: 'old', status: 'done' }),
    job({ id: 'a', status: 'done' }),
    job({ id: 'b', status: 'paused', pausedBy: 'storageFull' }),
    job({ id: 'c', status: 'paused', pausedBy: 'engineSwitch' }),
    job({ id: 'quick', status: 'done' }),
  ];
  assert.deepEqual(newlyEnded(seen, later).map((item) => [item.id, toastKindOf(item)]), [['a', 'ready'], ['b', 'stopped'], ['quick', 'ready']]);
});

test('durations read like the export panel and the toast show them', () => {
  assert.equal(formatDuration(20), '1 min');
  assert.equal(formatDuration(45 * 60), '45 min');
  assert.equal(formatDuration(8 * 3600), '8 h');
  assert.equal(formatDuration(8 * 3600 + 9.5 * 60), '8 h 10 min');
  assert.equal(formatDuration(m4bSeconds(8_000 * 3600)), '1 h', '64 kbit/s');
});

const job = (patch: Partial<AudioExport>): AudioExport => ({
  id: 'e1', documentId: 'd1', voiceId: 'af_heart', format: 'm4b', speedPercent: 100, engineId: 'kokoro-v1.0', range: null, rangeLabel: null, title: 'A Field Guide to Small Gardens',
  status: 'running', pausedBy: null, stage: 'parts', error: null, sentencesDone: 10, sentencesTotal: 100, partsDone: 0, partsTotal: 5, estimatedSecondsLeft: 60, fileBytes: null, location: null, waitingForEngine: false,
  ...patch,
});

test('the desktop export phase follows the exporter, the start request, the job and the save', () => {
  const idle = { kind: 'idle' } as const;
  assert.equal(desktopExportPhase(null, false, idle, false), 'unavailable');
  assert.equal(desktopExportPhase(null, true, idle, true), 'starting');
  assert.equal(desktopExportPhase(null, false, idle, true), 'idle');
  assert.equal(desktopExportPhase(job({ status: 'queued' }), false, idle, true), 'queued');
  assert.equal(desktopExportPhase(job({ waitingForEngine: true }), false, idle, true), 'waitingForModel');
  assert.equal(desktopExportPhase(job({ status: 'paused', pausedBy: 'storageFull' }), false, idle, true), 'storageFull');
  assert.equal(desktopExportPhase(job({ status: 'paused', pausedBy: 'system' }), false, idle, true), 'failed');
  assert.equal(desktopExportPhase(job({ status: 'done', location: { kind: 'appStorage', path: '/x' } }), false, idle, true), 'done');
  assert.equal(desktopExportPhase(job({ status: 'done', location: { kind: 'shared', uri: '/home/sam/Music/x.m4b' } }), false, idle, true), 'saved');
  assert.equal(desktopExportPhase(job({ status: 'done' }), false, { kind: 'moving' }, true), 'moving');
  assert.equal(desktopExportPhase(job({ status: 'done' }), false, { kind: 'failed', storageFull: true }, true), 'saveFailed');
  assert.equal(desktopExportPhase(job({ status: 'done' }), false, { kind: 'fileGone' }, true), 'fileGone');
  assert.equal(desktopExportPhase(job({ status: 'done' }), false, { kind: 'revealFailed' }, true), 'done');
  assert.equal(desktopExportPhase(job({ status: 'done', location: { kind: 'shared', uri: '/home/sam/Music/x.m4b' } }), false, { kind: 'revealFailed' }, true), 'saved');
});

test("a file manager that didn't start is shown until a reveal works, and keeps where the file was saved", () => {
  const inLectrice = job({ status: 'done', location: { kind: 'appStorage', path: '/x' } });
  const saved = { kind: 'saved', path: '/home/sam/Music/Book.m4b' } as const;

  const failedAfterSave = afterRevealFailed(saved);
  assert.deepEqual(saveStateOfEntry(failedAfterSave), { kind: 'revealFailed' });
  assert.equal(savedPathOfEntry(failedAfterSave, inLectrice), '/home/sam/Music/Book.m4b');
  assert.deepEqual(afterRevealed(failedAfterSave), saved);

  const failedInLectrice = afterRevealFailed(undefined);
  assert.equal(savedPathOfEntry(failedInLectrice, inLectrice), null);
  assert.equal(afterRevealed(failedInLectrice), undefined);
  assert.deepEqual(saveStateOfEntry(afterRevealFailed({ kind: 'failed', storageFull: true })), { kind: 'revealFailed' });

  const gone = { kind: 'fileGone' } as const;
  assert.equal(afterRevealed(gone), gone);
  assert.deepEqual(saveStateOfEntry(saved), { kind: 'idle' });
  assert.equal(savedPathOfEntry(undefined, job({ status: 'done', location: { kind: 'shared', uri: '/media/usb/Book.m4b' } })), '/media/usb/Book.m4b');
});

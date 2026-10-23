import { useMemo, useState } from 'react';
import { ChapterLine } from '../../components/ChapterLine/ChapterLine';
import { ChaptersRail, ChaptersRailHandle } from '../../components/ChaptersPanel/ChaptersRail';
import { ChaptersSheet } from '../../components/ChaptersPanel/ChaptersSheet';
import { ExportPanel } from '../../components/ExportPanel/ExportPanel';
import { MiniPlayer } from '../../components/MiniPlayer/MiniPlayer';
import { PlayerPopover } from '../../components/PlayerPopover/PlayerPopover';
import { ReadingView } from '../../components/ReadingView/ReadingView';
import { StatusBanner } from '../../components/StatusBanner/StatusBanner';
import { EXPORT_FORMAT_EXTENSION, FALLBACK_VOICE_ID, SECONDS_PER_CHAR } from '../../config/constants';
import { useAudioExport } from '../../hooks/useAudioExport';
import { useChaptersPanel } from '../../hooks/useChaptersPanel';
import { useFollowScroll } from '../../hooks/useFollowScroll';
import { usePlayback } from '../../hooks/usePlayback';
import { useReaderShortcuts } from '../../hooks/useReaderShortcuts';
import type { SpeechEngine } from '../../hooks/useSpeechEngine';
import type { Voices } from '../../hooks/useVoices';
import controls from '../../styles/controls.module.css';
import type { OpenedDocument } from '../../types/document';
import { exportPhase } from '../../utils/exportPhase';
import { safeFileName } from '../../utils/fileName';
import { buildPageLabelLookup, formatPageMarker, formatPdfPage, planPageMarkers, printedLabel } from '../../utils/pages';
import { secondsBetween } from '../../utils/playbackOrder';
import { playerState, playerStatusParts } from '../../utils/playerStatus';
import { sentenceIdEnd } from '../../utils/sentences';
import { formatTimeLeft } from '../../utils/time';
import { buildTocTree, chapterLabel, entryEndSentenceId, formatChapterPercent, percentThrough, tocLocation } from '../../utils/toc';
import { groupVoices } from '../../utils/voices';
import styles from './ReaderPage.module.css';

interface ReaderScreenProps { document: OpenedDocument; voices: Voices; engine: SpeechEngine; onBack: () => void; }

const POPOVER_ID = 'player-popover';
const SHEET_ID = 'chapters-sheet';

const ENGINE_BANNER: Record<SpeechEngine['state'], string | null> = {
  checking: null,
  ready: null,
  failed: 'Speech engine failed to load. Playback is unavailable until it is restarted.',
  unreachable: "Can't reach the Lectrice server. Is it running?",
};

/** A loaded document: wires playback, export and follow-scroll to the components. */
export const ReaderScreen = ({ document, voices, engine, onBack }: ReaderScreenProps) => {
  const voiceId = voices.selectedVoiceId ?? FALLBACK_VOICE_ID;
  const voiceNameOf = (id: string): string => voices.voices.find((voice) => voice.id === id)?.name ?? id;

  const playback = usePlayback({ document, voiceId, canPlay: engine.state === 'ready', onEngineUnavailable: engine.markUnavailable });
  const exportState = useAudioExport(document.id, engine.markUnavailable);
  const follow = useFollowScroll(playback.currentSentenceId, playback.currentBlockIndex);
  const chapters = useChaptersPanel(document.toc.length > 0);
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);

  const pageLabels = useMemo(() => buildPageLabelLookup(document.pages), [document.pages]);
  const markers = useMemo(() => planPageMarkers(document), [document]);
  const voiceGroups = useMemo(() => groupVoices(voices.voices), [voices.voices]);
  const tree = useMemo(() => buildTocTree(document.toc), [document.toc]);
  const sentenceCount = useMemo(() => sentenceIdEnd(document), [document]);
  const location = useMemo(() => tocLocation(document.toc, playback.currentSentenceId), [document.toc, playback.currentSentenceId]);

  // Every jump the reader asks for brings the view back to the current sentence.
  const andFollow = <Args extends unknown[]>(jump: (...args: Args) => void) => (...args: Args) => {
    jump(...args);
    follow.resumeFollowing();
  };
  const next = andFollow(playback.next);
  const previous = andFollow(playback.previous);
  const seek = andFollow(playback.seekToPosition);
  const selectSentence = andFollow(playback.jumpTo);
  const selectBlock = andFollow(playback.jumpToBlock);
  const selectFromSheet = (sentenceId: number) => {
    selectSentence(sentenceId);
    chapters.closeSheet();
  };
  const openChaptersFromPopover = () => {
    setIsPopoverOpen(false);
    chapters.openChapters();
  };
  useReaderShortcuts({ onTogglePlay: playback.togglePlay, onNext: next, onPrevious: previous, onToggleChapters: chapters.toggleWithKey });

  const currentLabel = playback.currentPageIndex === null ? '' : printedLabel(pageLabels, playback.currentPageIndex);
  const lastLabel = printedLabel(pageLabels, document.pageCount);
  const pageLabelOf = (pageIndex: number) => printedLabel(pageLabels, pageIndex);
  const state = playerState(engine.state, playback.status);
  const chapter = location === null ? null : document.toc[location.chapterIndex];
  const section = location === null || location.sectionIndex === null ? null : document.toc[location.sectionIndex];
  const statusParts = playerStatusParts(state, { pageText: formatPageMarker(currentLabel), lastPageText: lastLabel, chapter: chapter === null ? null : chapterLabel(chapter.title) }, playback.speed);
  const positionText = `Sentence ${(playback.position + 1).toLocaleString('en')} of ${playback.playableCount.toLocaleString('en')}, page ${currentLabel}`;
  const locationText = `Page ${currentLabel} · ${playback.currentPageIndex === null ? '' : formatPdfPage(playback.currentPageIndex)} of ${document.pageCount.toLocaleString('en')}`;

  const job = exportState.job;
  const fileFormat = job?.format ?? exportState.format;
  const fileVoiceName = voiceNameOf(job?.voiceId ?? voiceId);
  const downloadName = safeFileName(`${document.title} (${fileVoiceName}).${EXPORT_FORMAT_EXTENSION[fileFormat]}`);
  const engineMessage = ENGINE_BANNER[engine.state];

  const exportPanel = (
    <ExportPanel
      phase={exportPhase(job, exportState.isStarting)}
      format={exportState.format}
      onFormatChange={exportState.setFormat}
      fileFormat={fileFormat}
      fileVoiceName={fileVoiceName}
      sentencesDone={job?.sentencesDone ?? 0}
      sentencesTotal={job?.sentencesTotal ?? 0}
      timeLeftText={job?.estimatedSecondsLeft == null ? null : formatTimeLeft(job.estimatedSecondsLeft)}
      voiceName={voiceNameOf(voiceId)}
      readSkipped={playback.readSkipped}
      fileUrl={exportState.fileUrl}
      downloadName={downloadName}
      errorMessage={exportState.error?.message ?? null}
      canStart={engine.state === 'ready'}
      onStart={() => exportState.start(voiceId, playback.readSkipped)}
      onCancel={exportState.cancel}
    />
  );

  // `location` is non-null only for a sentence inside the toc, so the sentence id is non-null with it.
  const chapterLine = (chapterIndex: number, sentenceId: number) => {
    const chapterTitle = document.toc[chapterIndex].title;
    const pathText = section === null ? chapterTitle : `${chapterTitle} › ${section.title}`;
    const percentText = formatChapterPercent(percentThrough(document.toc, chapterIndex, sentenceId, sentenceCount));
    const secondsLeftInChapter = secondsBetween(playback.order, sentenceId, entryEndSentenceId(document.toc, chapterIndex), playback.speed, SECONDS_PER_CHAR);
    return <ChapterLine pathText={pathText} progressText={`${percentText} · ${formatTimeLeft(secondsLeftInChapter)} in it`} onOpen={openChaptersFromPopover} />;
  };
  const chapterSection = location === null || playback.currentSentenceId === null ? null : chapterLine(location.chapterIndex, playback.currentSentenceId);

  const popover = (
    <PlayerPopover
      id={POPOVER_ID}
      position={playback.position}
      playableCount={playback.playableCount}
      positionText={positionText}
      chapterSection={chapterSection}
      locationText={locationText}
      onOpenChapters={openChaptersFromPopover}
      timeLeftText={formatTimeLeft(playback.secondsLeft)}
      onSeek={seek}
      speed={playback.speed}
      onSpeedChange={playback.setSpeed}
      voiceGroups={voiceGroups}
      voiceId={voiceId}
      voicesAvailable={voices.status === 'ready'}
      onVoiceChange={voices.selectVoice}
      readSkipped={playback.readSkipped}
      onReadSkippedChange={playback.setReadSkipped}
      exportPanel={exportPanel}
      onOpenAnother={onBack}
    />
  );

  const tocProps = { toc: document.toc, tree, sentenceCount, currentSentenceId: playback.currentSentenceId, location, pageLabelOf, bookTitle: document.title };

  return (
    <>
      {chapters.railShown && <ChaptersRail {...tocProps} focusRequest={chapters.railFocusRequest} onSelect={selectSentence} onClose={chapters.closeRail} onLeave={chapters.returnFocus} />}
      {chapters.railClosed && <ChaptersRailHandle onOpen={chapters.openRail} />}
      <div className={chapters.railShown ? styles.besideRail : undefined}>
        <div className={styles.banners}>
          {engineMessage && <StatusBanner message={engineMessage} actions={<button type="button" className={controls.button} onClick={engine.recheck}>Retry</button>} />}
          {playback.audioError && (
            <StatusBanner
              message={`Couldn't make audio for this sentence. ${playback.audioError.message}`}
              actions={(
                <>
                  <button type="button" className={controls.button} onClick={playback.retryAudio}>Try again</button>
                  <button type="button" className={controls.button} onClick={playback.skipFailedSentence}>Skip sentence</button>
                </>
              )}
            />
          )}
        </div>
        <ReadingView
          document={document}
          activeSentenceId={playback.currentSentenceId}
          activeBlockIndex={playback.currentBlockIndex}
          isPreparing={playback.status === 'preparing'}
          readSkipped={playback.readSkipped}
          markers={markers}
          pageLabels={pageLabels}
          onSentenceSelect={selectSentence}
          onBlockSelect={selectBlock}
        />
      </div>
      <MiniPlayer
        playerState={state}
        statusParts={statusParts}
        canGoBack={playback.position > 0}
        canGoForward={playback.position < playback.playableCount - 1}
        showBackToCurrent={!follow.isFollowing}
        onTogglePlay={playback.togglePlay}
        onPrevious={previous}
        onNext={next}
        onBackToCurrent={follow.resumeFollowing}
        popover={popover}
        popoverId={POPOVER_ID}
        isPopoverOpen={isPopoverOpen}
        onPopoverOpenChange={setIsPopoverOpen}
        showChaptersButton={!chapters.railShown}
        isChaptersOpen={chapters.isSheetOpen}
        chaptersPanelId={SHEET_ID}
        onToggleChapters={chapters.toggleSheet}
      />
      {chapters.isSheetOpen && <ChaptersSheet key={String(chapters.isSheetModal)} id={SHEET_ID} {...tocProps} isModal={chapters.isSheetModal} onSelect={selectFromSheet} onClose={chapters.closeSheet} />}
    </>
  );
};

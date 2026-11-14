import { useEffect, useMemo, useRef, useState } from 'react';
import { ChapterLine } from '../../components/ChapterLine/ChapterLine';
import { ChaptersRail, ChaptersRailHandle } from '../../components/ChaptersPanel/ChaptersRail';
import { ChaptersSheet } from '../../components/ChaptersPanel/ChaptersSheet';
import { MiniPlayer } from '../../components/MiniPlayer/MiniPlayer';
import { PlayerPopover } from '../../components/PlayerPopover/PlayerPopover';
import { ReadingView } from '../../components/ReadingView/ReadingView';
import { StatusBanner } from '../../components/StatusBanner/StatusBanner';
import { SECONDS_PER_CHAR } from '../../config/constants';
import { useAudioExport } from '../../hooks/useAudioExport';
import { useChaptersPanel } from '../../hooks/useChaptersPanel';
import { useFollowScroll } from '../../hooks/useFollowScroll';
import { usePlayback } from '../../hooks/usePlayback';
import { useReaderShortcuts } from '../../hooks/useReaderShortcuts';
import type { SpeechEngine } from '../../hooks/useSpeechEngine';
import type { Voices } from '../../hooks/useVoices';
import controls from '../../styles/controls.module.css';
import type { OpenedDocument } from '../../types/document';
import { buildPageLabelLookup, planPageMarkers, printedLabel } from '../../utils/pages';
import { playerState } from '../../utils/playerStatus';
import { sentenceIdEnd } from '../../utils/sentences';
import { formatTimeLeft } from '../../utils/time';
import { buildTocTree, tocLocation } from '../../utils/toc';
import { groupVoices } from '../../utils/voices';
import { ReaderExport } from './ReaderExport';
import styles from './ReaderPage.module.css';
import type { ReaderShell } from './readerShell';
import { chapterLine, locationText, positionText, statusParts } from './readerText';

interface ReaderScreenProps { document: OpenedDocument; voices: Voices; engine: SpeechEngine; onBack: () => void; shell?: ReaderShell; }

const POPOVER_ID = 'player-popover';
const SHEET_ID = 'chapters-sheet';

const ENGINE_BANNER: Record<SpeechEngine['state'], string | null> = {
  checking: null,
  ready: null,
  failed: 'Speech engine failed to load. Playback is unavailable until it is restarted.',
  unreachable: "Can't reach the Lectrice server. Is it running?",
};

/** A loaded document: wires playback, export and follow-scroll to the components. */
export const ReaderScreen = ({ document, voices, engine, onBack, shell }: ReaderScreenProps) => {
  const voiceId = voices.selectedVoiceId;
  const voiceNameOf = (id: string | null): string => (id === null ? '' : voices.voices.find((voice) => voice.id === id)?.name ?? id);
  const canStartExport = engine.state === 'ready' && voiceId !== null;

  const playback = usePlayback({ document, voiceId, canPlay: engine.state === 'ready', onEngineUnavailable: engine.markUnavailable });
  const exportState = useAudioExport(document.id, engine.markUnavailable);
  const follow = useFollowScroll(playback.currentSentenceId, playback.currentBlockIndex);
  const chapters = useChaptersPanel(document.toc.length > 0);
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const shellExportRef = useRef<HTMLDivElement>(null);

  const onRailShownChange = shell?.onRailShownChange;
  const { railShown } = chapters;
  useEffect(() => {
    if (!onRailShownChange) return;
    onRailShownChange(railShown);
    return () => onRailShownChange(false);
  }, [railShown, onRailShownChange]);

  // The popover opens in this render; its export section is scrolled into view once it is on screen.
  const openAtExport = shell?.openAtExport ?? false;
  const onOpenedAtExport = shell?.onOpenedAtExport;
  if (openAtExport && !isPopoverOpen) setIsPopoverOpen(true);
  useEffect(() => {
    if (!openAtExport) return;
    shellExportRef.current?.scrollIntoView({ block: 'nearest' });
    onOpenedAtExport?.();
  }, [openAtExport, onOpenedAtExport]);

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
  const voiceStatus = shell?.voiceStatus ?? null;
  const engineMessage = voiceStatus === null ? ENGINE_BANNER[engine.state] : null;
  const pillStatus = statusParts({ state, voiceStatus, pageLabel: currentLabel, lastPageLabel: lastLabel, chapterTitle: location === null ? null : document.toc[location.chapterIndex].title, speed: playback.speed });

  const exportPanel = (
    <ReaderExport
      documentTitle={document.title}
      exportState={exportState}
      voiceName={voiceNameOf(voiceId)}
      fileVoiceName={voiceNameOf(exportState.job?.voiceId ?? voiceId)}
      readSkipped={playback.readSkipped}
      canStart={canStartExport}
      onStart={() => {
        if (voiceId !== null) exportState.start(voiceId, playback.readSkipped);
      }}
      bookSeconds={playback.order.charsFrom[0] * SECONDS_PER_CHAR}
      shell={shell}
      shellPanelRef={shellExportRef}
    />
  );

  // `location` is non-null only for a sentence inside the toc, so the sentence id is non-null with it.
  const chapterSection = location === null || playback.currentSentenceId === null
    ? null
    : <ChapterLine {...chapterLine({ toc: document.toc, location, sentenceId: playback.currentSentenceId, sentenceCount, order: playback.order, speed: playback.speed })} onOpen={openChaptersFromPopover} />;

  const popover = (
    <PlayerPopover
      id={POPOVER_ID}
      position={playback.position}
      playableCount={playback.playableCount}
      positionText={positionText(playback.position, playback.playableCount, currentLabel)}
      chapterSection={chapterSection}
      locationText={locationText(currentLabel, playback.currentPageIndex, document.pageCount)}
      onOpenChapters={openChaptersFromPopover}
      timeLeftText={formatTimeLeft(playback.secondsLeft)}
      onSeek={seek}
      speed={playback.speed}
      onSpeedChange={playback.setSpeed}
      voiceGroups={voiceGroups}
      voiceId={voiceId ?? ''}
      voicesAvailable={voices.status === 'ready'}
      voicesFailed={voices.status === 'failed'}
      onRetryVoices={voices.retry}
      onVoiceChange={voices.selectVoice}
      readSkipped={playback.readSkipped}
      onReadSkippedChange={playback.setReadSkipped}
      exportPanel={exportPanel}
      onOpenAnother={onBack}
      noVoiceNotice={shell?.noVoiceNotice}
      extraActions={shell?.extraActions}
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
        statusParts={pillStatus}
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

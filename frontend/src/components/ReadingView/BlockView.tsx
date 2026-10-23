import { Fragment, memo } from 'react';
import type { Block, HeadingLevel, Sentence as SentenceModel, SkippedBlock } from '../../types/document';
import { printedLabel } from '../../utils/pages';
import type { PageMarkerPlan } from '../../utils/pages';
import { PageMarker } from '../PageMarker/PageMarker';
import { Sentence } from '../Sentence/Sentence';
import type { SentenceState } from '../Sentence/Sentence';
import { SkippedBlockView } from '../SkippedBlockView/SkippedBlockView';
import type { SkippedBlockState } from '../SkippedBlockView/SkippedBlockView';
import styles from './ReadingView.module.css';

interface BlockViewProps {
  block: Block;
  blockIndex: number;
  isActive: boolean;
  /** Only set on the active block; null for every other one, so they never re-render as playback moves. */
  activeSentenceId: number | null;
  isRead: boolean;
  isPreparing: boolean;
  readSkipped: boolean;
  markers: PageMarkerPlan;
  pageLabels: ReadonlyMap<number, string>;
}

const HEADING_TAG: Record<HeadingLevel, 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6'> = { 1: 'h1', 2: 'h2', 3: 'h3', 4: 'h4', 5: 'h5', 6: 'h6' };
const HEADING_CLASS: Record<HeadingLevel, string> = { 1: styles.heading1, 2: styles.heading2, 3: styles.headingSmall, 4: styles.headingSmall, 5: styles.headingSmall, 6: styles.headingSmall };
const PARAGRAPH_CLASS: Record<'paragraph' | 'listItem' | 'note', string> = { paragraph: styles.paragraph, listItem: styles.listItem, note: styles.note };

const isSkipped = (block: Block): block is SkippedBlock => 'spoken' in block;

const sentenceState = (sentence: SentenceModel, activeSentenceId: number | null, isRead: boolean, isPreparing: boolean): SentenceState => {
  if (sentence.id === activeSentenceId) return isPreparing ? 'activePreparing' : 'active';
  // Ids are sequential, so earlier sentences of the active block are read too.
  if (isRead || (activeSentenceId !== null && sentence.id < activeSentenceId)) return 'read';
  return 'unread';
};

const blockState = (isActive: boolean, isRead: boolean, isPreparing: boolean): SkippedBlockState => {
  if (isActive) return isPreparing ? 'activePreparing' : 'active';
  return isRead ? 'read' : 'unread';
};

export const BlockView = memo(({ block, blockIndex, isActive, activeSentenceId, isRead, isPreparing, readSkipped, markers, pageLabels }: BlockViewProps) => {
  const markerFor = (pageIndex: number) => <PageMarker label={printedLabel(pageLabels, pageIndex)} pageIndex={pageIndex} />;

  if (isSkipped(block)) {
    const firstSentence = block.sentences[0];
    const hasMarker = block.kind === 'caption' ? firstSentence !== undefined && markers.beforeSentence.has(firstSentence.id) : markers.beforeBlock.has(block.id);
    return (
      <div className={styles.block} data-block-index={blockIndex}>
        <SkippedBlockView block={block} blockIndex={blockIndex} state={blockState(isActive, isRead, isPreparing)} activeSentenceId={activeSentenceId} isPreparing={isPreparing} readSkipped={readSkipped} marker={hasMarker ? markerFor(block.pageIndex) : null} />
      </div>
    );
  }

  const sentences = block.sentences.map((sentence, index) => (
    <Fragment key={sentence.id}>
      {index > 0 && ' '}
      {markers.beforeSentence.has(sentence.id) && markerFor(sentence.pageIndex)}
      <Sentence sentence={sentence} state={sentenceState(sentence, activeSentenceId, isRead, isPreparing)} />
    </Fragment>
  ));

  if (block.kind === 'heading') {
    const Heading = HEADING_TAG[block.level];
    return <div className={styles.block} data-block-index={blockIndex}><Heading className={HEADING_CLASS[block.level]}>{sentences}</Heading></div>;
  }
  return <div className={styles.block} data-block-index={blockIndex}><p className={PARAGRAPH_CLASS[block.kind]}>{sentences}</p></div>;
});

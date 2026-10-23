import type { SkippedBlock } from '../../types/document';
import { Sentence } from '../Sentence/Sentence';
import type { SentenceState } from '../Sentence/Sentence';
import styles from './SkippedBlockView.module.css';

export type SkippedBlockState = 'unread' | 'read' | 'active' | 'activePreparing';

interface SkippedBlockViewProps {
  block: SkippedBlock;
  blockIndex: number;
  /** For code and tables the whole box is the "sentence". */
  state: SkippedBlockState;
  /** Captions highlight their own sentence spans. */
  activeSentenceId: number | null;
  isPreparing: boolean;
  readSkipped: boolean;
  /** The PageMarker to place first, or null. */
  marker: React.ReactNode;
}

const BOX_LABEL: Record<'code' | 'table', string> = { code: 'Code', table: 'Table' };

const BOX_CLASS: Record<SkippedBlockState, string> = {
  unread: styles.box,
  read: styles.box,
  active: `${styles.box} ${styles.active}`,
  activePreparing: `${styles.box} ${styles.active} ${styles.activePreparing}`,
};

const readingNote = (block: SkippedBlock, readSkipped: boolean): string => {
  if (readSkipped) return 'read aloud';
  return block.cue ? `says “${block.cue.text}”` : 'not read aloud';
};

const captionSentenceState = (sentenceId: number, activeSentenceId: number | null, blockState: SkippedBlockState, isPreparing: boolean): SentenceState => {
  if (sentenceId === activeSentenceId) return isPreparing ? 'activePreparing' : 'active';
  if (blockState === 'read' || (activeSentenceId !== null && sentenceId < activeSentenceId)) return 'read';
  return 'unread';
};

export const SkippedBlockView = ({ block, blockIndex, state, activeSentenceId, isPreparing, readSkipped, marker }: SkippedBlockViewProps) => {
  if (block.kind === 'caption') {
    return (
      <p className={styles.caption}>
        {marker}
        {block.sentences.map((sentence, index) => (
          <span key={sentence.id}>
            {index > 0 && ' '}
            <Sentence sentence={sentence} state={captionSentenceState(sentence.id, activeSentenceId, state, isPreparing)} />
          </span>
        ))}
      </p>
    );
  }
  return (
    <figure className={BOX_CLASS[state]} data-jump-block-index={blockIndex}>
      {marker}
      <figcaption className={styles.label}>{BOX_LABEL[block.kind]} · {readingNote(block, readSkipped)}</figcaption>
      <pre className={styles.pre}><code>{block.text}</code></pre>
    </figure>
  );
};

import type { CleanDocument } from '../../types/document';
import type { PageMarkerPlan } from '../../utils/pages';
import { BlockView } from './BlockView';
import styles from './ReadingView.module.css';

interface ReadingViewProps {
  document: CleanDocument;
  activeSentenceId: number | null;
  activeBlockIndex: number | null;
  isPreparing: boolean;
  readSkipped: boolean;
  markers: PageMarkerPlan;
  pageLabels: ReadonlyMap<number, string>;
  /** Any sentence id; playback resolves it onto the current order. */
  onSentenceSelect: (sentenceId: number) => void;
  /** A click on a code or table box. */
  onBlockSelect: (blockIndex: number) => void;
}

/** The article. One delegated click handler instead of one per sentence. */
export const ReadingView = ({ document, activeSentenceId, activeBlockIndex, isPreparing, readSkipped, markers, pageLabels, onSentenceSelect, onBlockSelect }: ReadingViewProps) => {
  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    if (!(event.target instanceof Element)) return;
    // The reader is selecting text to copy, not asking to jump.
    if (window.getSelection()?.toString()) return;
    const sentence = event.target.closest('[data-sentence-id]');
    if (sentence instanceof HTMLElement) {
      onSentenceSelect(Number(sentence.dataset.sentenceId));
      return;
    }
    const box = event.target.closest('[data-jump-block-index]');
    if (box instanceof HTMLElement) onBlockSelect(Number(box.dataset.jumpBlockIndex));
  };

  return (
    <article className={styles.article} onClick={handleClick}>
      {document.blocks.map((block, blockIndex) => {
        const isActive = blockIndex === activeBlockIndex;
        return (
          <BlockView
            key={block.id}
            block={block}
            blockIndex={blockIndex}
            isActive={isActive}
            activeSentenceId={isActive ? activeSentenceId : null}
            isRead={activeBlockIndex !== null && blockIndex < activeBlockIndex}
            isPreparing={isActive && isPreparing}
            readSkipped={readSkipped}
            markers={markers}
            pageLabels={pageLabels}
          />
        );
      })}
    </article>
  );
};

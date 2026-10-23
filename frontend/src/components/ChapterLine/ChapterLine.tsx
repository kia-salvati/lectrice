import styles from './ChapterLine.module.css';

interface ChapterLineProps {
  /** "7. Long Documents › 7.3 Tables of contents that tell the truth" */
  pathText: string;
  /** "38% of this chapter · 9 min left in it" */
  progressText: string;
  onOpen: () => void;
}

/** The popover's chapter line: where the reader is in the book's outline; clicking it opens the chapters. */
export const ChapterLine = ({ pathText, progressText, onOpen }: ChapterLineProps) => (
  <div className={styles.line}>
    <button type="button" className={styles.path} title="Open the chapters" onClick={onOpen}>{pathText}</button>
    <div className={styles.progress}>{progressText}</div>
  </div>
);

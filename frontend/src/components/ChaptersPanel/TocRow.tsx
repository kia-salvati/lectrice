import { formatPageMarker } from '../../utils/pages';
import type { EntryProgress, TocNode } from '../../utils/toc';
import { CheckIcon } from '../icons/icons';
import styles from './ChaptersPanel.module.css';

/** Columns: `tree` = disclosure | title | meta (the rail), `plain` = title | meta (parts, flat lists), `drill` = title | meta | drill button (the sheet). */
export type RowLayout = 'tree' | 'plain' | 'drill';

/** The progress bar under a chapter's title: how much of it is read, and (rail only) how long it is next to the other chapters. */
export interface RowBar { fillPercent: number; widthPercent: number | null; }

interface TocRowProps {
  node: TocNode;
  layout: RowLayout;
  pageLabel: string;
  progress: EntryProgress;
  /** The current entry itself (the deepest one on the path). */
  isHere: boolean;
  isPart: boolean;
  /** Shown in the meta for the current chapter. */
  percent: number | null;
  bar: RowBar | null;
  /** "6 sections", under the title of a collapsed chapter. */
  sizeText: string | null;
  /** The disclosure button or its spacer (`tree` layout). */
  leading?: React.ReactNode;
  /** The drill button (`drill` layout). */
  trailing?: React.ReactNode;
  onSelect: (sentenceId: number) => void;
}

const LAYOUT_CLASS: Record<RowLayout, string> = { tree: styles.tree, plain: styles.plain, drill: styles.drill };
const PROGRESS_CLASS: Record<EntryProgress, string> = { read: styles.read, current: styles.current, unread: '' };
/** Spoken after the title and page, so a screen reader hears the state without the colour. */
const PROGRESS_TEXT: Record<EntryProgress, string> = { read: ', read', current: ', current', unread: '' };

/** One table-of-contents line. `data-toc-index` on the title button is what the panels' keyboard handlers navigate by. */
export const TocRow = ({ node, layout, pageLabel, progress, isHere, isPart, percent, bar, sizeText, leading, trailing, onSelect }: TocRowProps) => {
  const { entry } = node;
  const className = [styles.row, LAYOUT_CLASS[layout], PROGRESS_CLASS[progress], isPart ? styles.part : ''].filter(Boolean).join(' ');
  return (
    <div className={className} data-toc-row>
      {leading}
      <button type="button" className={styles.title} data-toc-index={node.index} aria-current={isHere ? 'location' : undefined} onClick={() => onSelect(entry.sentenceId)}>
        <span className={styles.titleText} title={entry.title}>{entry.title}</span>
        {sizeText !== null && <span className={styles.size}>{sizeText}</span>}
        {bar !== null && (
          <span className={styles.bar} style={bar.widthPercent === null ? undefined : { width: `${bar.widthPercent}%` }}>
            <span className={styles.fill} style={{ width: `${bar.fillPercent}%` }} />
          </span>
        )}
        <span className={styles.srOnly}>, page {pageLabel}{PROGRESS_TEXT[progress]}</span>
      </button>
      <span className={styles.meta}>
        {percent !== null && <span>{percent}%</span>}
        {progress === 'read' && <CheckIcon />}
        <span>{formatPageMarker(pageLabel)}</span>
      </span>
      {trailing}
    </div>
  );
};

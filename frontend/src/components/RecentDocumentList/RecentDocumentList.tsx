import controls from '../../styles/controls.module.css';
import type { DocumentId, DocumentSummary } from '../../types/document';
import { percentRead } from '../../utils/progress';
import { formatRelativeTime } from '../../utils/time';
import styles from './RecentDocumentList.module.css';

interface RecentDocumentListProps {
  documents: DocumentSummary[];
  /** Passed in so the relative times stay a pure function of their inputs. */
  now: Date;
  hrefFor: (documentId: DocumentId) => string;
  hasMore: boolean;
  isLoadingMore: boolean;
  onShowMore: () => void;
}

const HEADING_ID = 'recent-documents-heading';

export const RecentDocumentList = ({ documents, now, hrefFor, hasMore, isLoadingMore, onShowMore }: RecentDocumentListProps) => (
  <section aria-labelledby={HEADING_ID}>
    <h2 id={HEADING_ID} className={styles.heading}>Recent documents</h2>
    <ul className={styles.list}>
      {documents.map((summary) => {
        const percent = percentRead(summary.lastSentenceId, summary.sentenceCount);
        const action = percent === null ? 'Start' : `Continue · ${percent}%`;
        return (
          <li key={summary.id}>
            <a className={styles.row} href={hrefFor(summary.id)}>
              <span className={styles.main}>
                <span className={styles.title}>{summary.title.trim() || summary.sourceFileName}</span>
                <span className={styles.meta}>{summary.pageCount.toLocaleString('en')} pages · opened {formatRelativeTime(summary.lastOpenedAt, now)}</span>
              </span>
              <span className={styles.side}>
                <span className={styles.action}>{action}</span>
                {percent !== null && <span className={styles.track} aria-hidden="true"><span className={styles.fill} style={{ width: `${percent}%` }} /></span>}
              </span>
            </a>
          </li>
        );
      })}
    </ul>
    {hasMore && (
      <button type="button" className={`${controls.button} ${styles.more}`} disabled={isLoadingMore} onClick={onShowMore}>{isLoadingMore ? 'Loading…' : 'Show more'}</button>
    )}
  </section>
);

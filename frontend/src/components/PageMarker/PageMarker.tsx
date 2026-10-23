import { formatPageMarker, formatPdfPage } from '../../utils/pages';
import styles from './PageMarker.module.css';

interface PageMarkerProps { label: string; pageIndex: number; }

/** "p. 37" in the margin (inline chip on phones). Not selectable, so copied text stays clean; never a sentence, so never spoken. */
export const PageMarker = ({ label, pageIndex }: PageMarkerProps) => (
  <span className={styles.marker} title={formatPdfPage(pageIndex)}>{formatPageMarker(label)}</span>
);

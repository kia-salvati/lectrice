import styles from './LibraryHeader.module.css';

interface LibraryHeaderProps {
  /** Right-aligned controls (the desktop's Settings button). */
  actions?: React.ReactNode;
}

/** Brand and tagline over the library and the desktop's first-run screen. */
export const LibraryHeader = ({ actions }: LibraryHeaderProps) => (
  <header className={styles.header}>
    <h1 className={styles.brand}>Lectrice</h1>
    <span className={styles.tagline}>Reads PDFs aloud, locally</span>
    {actions && <div className={styles.actions}>{actions}</div>}
  </header>
);

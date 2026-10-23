import styles from './ChaptersPanel.module.css';

/** A book whose PDF had neither bookmarks nor headings the cleaner could use. */
export const ChaptersEmpty = () => <p className={styles.empty}>No chapters found. This PDF has no bookmarks or headings Lectrice could detect. Use the progress slider in ⋯ to move around.</p>;

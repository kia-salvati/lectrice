import { PATH_TAIL_LENGTH } from '../../config';
import styles from './PathText.module.css';

/** A file path in mono that shortens in the middle, so the file name's end stays readable. */
export const PathText = ({ path }: { path: string }) => {
  const cut = Math.max(0, path.length - PATH_TAIL_LENGTH);
  return (
    <span className={styles.path} title={path}>
      <span className={styles.head}>{path.slice(0, cut)}</span>
      <span className={styles.tail}>{path.slice(cut)}</span>
    </span>
  );
};

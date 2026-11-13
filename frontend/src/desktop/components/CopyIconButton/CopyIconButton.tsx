import { CheckIcon } from '../../../components/icons/icons';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import desktop from '../../styles/controls.module.css';
import { CopyIcon } from '../icons';
import styles from './CopyIconButton.module.css';

const noop = () => {};

/** A small Copy button beside a text that is already on screen; shows a check and "Copied" for a moment. */
export const CopyIconButton = ({ text }: { text: string }) => {
  const { copied, copy } = useCopyFeedback(noop);
  return (
    <>
      <button type="button" className={styles.button} aria-label="Copy" title="Copy" onClick={() => copy(text)}>{copied ? <CheckIcon /> : <CopyIcon />}</button>
      <span className={desktop.meta} role="status">{copied ? 'Copied' : ''}</span>
    </>
  );
};

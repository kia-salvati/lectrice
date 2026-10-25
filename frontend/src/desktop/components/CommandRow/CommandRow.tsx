import { useCallback, useRef } from 'react';
import controls from '../../../styles/controls.module.css';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import styles from './CommandRow.module.css';

interface CommandRowProps { label: string; command: string; }

/** A label over a shell command and its Copy button ("Copied" for two seconds; the text is selected if the clipboard refuses). */
export const CommandRow = ({ label, command }: CommandRowProps) => {
  const codeRef = useRef<HTMLElement>(null);
  const selectCommand = useCallback(() => {
    const code = codeRef.current;
    if (!code) return;
    const range = document.createRange();
    range.selectNodeContents(code);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  }, []);
  const { copied, copy } = useCopyFeedback(selectCommand);

  return (
    <div className={styles.command}>
      <span className={styles.label}>{label}</span>
      <div className={styles.row}>
        <code ref={codeRef} className={styles.code}>{command}</code>
        <button type="button" className={controls.button} onClick={() => copy(command)}>{copied ? 'Copied' : 'Copy'}</button>
      </div>
    </div>
  );
};

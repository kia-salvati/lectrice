import { useEffect, useRef, useState } from 'react';
import { COMPUTER_NAME_MAX_LENGTH } from '../../../config';
import desktop from '../../../styles/controls.module.css';
import styles from '../PhoneTab.module.css';

/** This computer's name: a text with "Rename", or the input (Enter saves, Escape or leaving the field cancels). */
export const ComputerName = ({ name, onRename }: { name: string; onRename: (name: string) => void }) => {
  const [draft, setDraft] = useState<string | null>(null);
  const renameRef = useRef<HTMLButtonElement>(null);
  /** The field closed with the keyboard's focus still in it: the focus goes back to "Rename" instead of getting lost. */
  const focusRename = useRef(false);
  const editing = draft !== null;
  useEffect(() => {
    if (editing || !focusRename.current) return;
    focusRename.current = false;
    renameRef.current?.focus();
  }, [editing]);

  const close = () => {
    focusRename.current = true;
    setDraft(null);
  };
  if (draft === null) {
    return <><span>{name}</span><button type="button" ref={renameRef} className={desktop.textButton} onClick={() => setDraft(name)}>Rename</button></>;
  }
  const handleKey = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      // Escape would also close the settings dialog.
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === 'Enter') {
      const trimmed = draft.trim();
      if (trimmed !== '' && trimmed !== name) onRename(trimmed);
      close();
    }
  };
  return (
    <input
      type="text"
      className={styles.field}
      aria-label="This computer's name"
      maxLength={COMPUTER_NAME_MAX_LENGTH}
      value={draft}
      autoFocus
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={handleKey}
      onBlur={(event) => {
        // Left by clicking or tabbing to another control: the focus stays where the user put it.
        if (event.relatedTarget === null) close();
        else setDraft(null);
      }}
    />
  );
};

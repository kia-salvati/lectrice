import { useRef } from 'react';
import type { ImportState } from '../../hooks/useLibrary';
import controls from '../../styles/controls.module.css';
import styles from './DropZone.module.css';
import { importErrorCopy } from './importErrorCopy';

interface DropZoneProps {
  importState: ImportState;
  isDragging: boolean;
  maxUploadMb: number;
  onFileChosen: (file: File) => void;
  onChooseAnother: () => void;
}

export const DropZone = ({ importState, isDragging, maxUploadMb, onFileChosen, onChooseAnother }: DropZoneProps) => {
  const inputRef = useRef<HTMLInputElement>(null);

  if (importState.phase === 'importing') {
    return (
      <div className={styles.drop} aria-live="polite">
        <h2 className={styles.title}>Reading {importState.fileName}…</h2>
        <p className={styles.text}>Extracting text and cleaning out headers, page numbers, footnotes and references.</p>
        <progress className={styles.progress} aria-label="Preparing document" />
      </div>
    );
  }

  if (importState.phase === 'failed') {
    const copy = importErrorCopy(importState.error);
    return (
      <div className={`${styles.drop} ${styles.err}`} role="alert">
        <h2 className={styles.title}>{copy.title}</h2>
        <p className={styles.text}><b>{importState.fileName}</b> {copy.body}</p>
        <button type="button" className={controls.button} onClick={onChooseAnother}>Choose another PDF</button>
      </div>
    );
  }

  return (
    <div className={isDragging ? `${styles.drop} ${styles.dragging}` : styles.drop}>
      <h2 className={styles.title}>Open a PDF to start listening</h2>
      <p className={styles.text}>Lectrice extracts the text, removes running headers, page numbers, footnotes, citation markers and reference lists, and reads what is left aloud with a local voice. Nothing leaves this computer.</p>
      <button type="button" className={`${controls.button} ${controls.primary}`} onClick={() => inputRef.current?.click()}>Open PDF…</button>
      <p className={styles.muted}>or drop a file anywhere in this window · PDFs with a text layer, up to {maxUploadMb} MB</p>
      <input
        ref={inputRef}
        type="file"
        accept="application/pdf,.pdf"
        hidden
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) onFileChosen(file);
          // Reset so choosing the same file again still fires a change.
          event.target.value = '';
        }}
      />
    </div>
  );
};

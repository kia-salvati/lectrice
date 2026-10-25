import { useCallback, useEffect, useState } from 'react';
import { COPIED_FEEDBACK_MS } from '../config';

/** Copies a text to the clipboard and reports "copied" for two seconds. On failure the caller selects the text instead. */
export const useCopyFeedback = (onCopyFailed: () => void): { copied: boolean; copy: (text: string) => void } => {
  const [copiedAt, setCopiedAt] = useState<number | null>(null);

  useEffect(() => {
    if (copiedAt === null) return;
    const timer = setTimeout(() => setCopiedAt(null), COPIED_FEEDBACK_MS);
    return () => clearTimeout(timer);
  }, [copiedAt]);

  const copy = useCallback((text: string) => {
    const done = () => setCopiedAt(Date.now());
    try {
      navigator.clipboard.writeText(text).then(done, onCopyFailed);
    } catch (error) {
      console.warn('Clipboard unavailable', error);
      onCopyFailed();
    }
  }, [onCopyFailed]);

  return { copied: copiedAt !== null, copy };
};

import { useEffect } from 'react';
import { hasErrorName } from '../../api/errors';
import { useDocument } from '../../hooks/useDocument';
import { useSpeechEngine } from '../../hooks/useSpeechEngine';
import { useVoices } from '../../hooks/useVoices';
import centered from '../../styles/centered.module.css';
import controls from '../../styles/controls.module.css';
import type { DocumentId } from '../../types/document';
import type { ReaderShell } from './readerShell';
import { ReaderScreen } from './ReaderScreen';

interface ReaderPageProps { documentId: DocumentId; onBack: () => void; shell?: ReaderShell; }

const APP_TITLE = 'Lectrice';

export const ReaderPage = ({ documentId, onBack, shell }: ReaderPageProps) => {
  // Three independent requests, started together.
  const load = useDocument(documentId);
  const voices = useVoices();
  const engine = useSpeechEngine();

  const title = load.document?.title;
  useEffect(() => {
    if (title) document.title = `${title} · ${APP_TITLE}`;
    return () => {
      document.title = APP_TITLE;
    };
  }, [title]);

  const backButton = <button type="button" className={controls.button} onClick={onBack}>Back to library</button>;

  if (load.status === 'loading') return <div className={centered.centered}><p className={centered.text} role="status">Opening document…</p></div>;

  if (load.status === 'failed' || load.document === null) {
    if (hasErrorName(load.error, 'DocumentNotFound')) {
      return (
        <div className={centered.centered}>
          <h1 className={centered.title}>Document not found</h1>
          <p className={centered.text}>It may have been removed from the library.</p>
          <div className={centered.actions}>{backButton}</div>
        </div>
      );
    }
    return (
      <div className={centered.centered}>
        <p className={centered.text} role="alert">{load.error?.message ?? 'The document could not be opened.'}</p>
        <div className={centered.actions}>
          <button type="button" className={`${controls.button} ${controls.primary}`} onClick={load.retry}>Try again</button>
          {backButton}
        </div>
      </div>
    );
  }

  return <ReaderScreen document={load.document} voices={voices} engine={engine} onBack={onBack} shell={shell} />;
};

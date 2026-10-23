import { useState } from 'react';
import { DropZone } from '../../components/DropZone/DropZone';
import { RecentDocumentList } from '../../components/RecentDocumentList/RecentDocumentList';
import { StatusBanner } from '../../components/StatusBanner/StatusBanner';
import { MAX_UPLOAD_MB } from '../../config/constants';
import { routeHref } from '../../config/routes';
import { useLibrary } from '../../hooks/useLibrary';
import type { Library } from '../../hooks/useLibrary';
import controls from '../../styles/controls.module.css';
import type { DocumentId } from '../../types/document';
import styles from './LibraryPage.module.css';

interface LibraryPageProps { onOpenDocument: (documentId: DocumentId) => void; }

const hrefFor = (documentId: DocumentId): string => routeHref({ name: 'reader', documentId });

const RecentSection = ({ library }: { library: Library }) => {
  if (library.recentStatus === 'loading') return <p className={styles.message}>Loading recent documents…</p>;
  if (library.recentStatus === 'failed') {
    return <StatusBanner message={library.recentError?.message ?? 'Recent documents could not be loaded.'} actions={<button type="button" className={controls.button} onClick={library.reloadRecent}>Try again</button>} />;
  }
  if (library.recent.length === 0) return <p className={styles.message}>No documents yet. Open a PDF to start.</p>;
  return <RecentDocumentList documents={library.recent} now={library.recentLoadedAt} hrefFor={hrefFor} hasMore={library.recent.length < library.total} isLoadingMore={library.isLoadingMore} onShowMore={library.loadMore} />;
};

export const LibraryPage = ({ onOpenDocument }: LibraryPageProps) => {
  const library = useLibrary({ onDocumentReady: onOpenDocument });
  // A counter, because dragleave fires every time the pointer crosses a child element.
  const [dragDepth, setDragDepth] = useState(0);

  const handleDragEnter = (event: React.DragEvent) => {
    event.preventDefault();
    setDragDepth((depth) => depth + 1);
  };
  const handleDragOver = (event: React.DragEvent) => event.preventDefault();
  const handleDragLeave = () => setDragDepth((depth) => Math.max(0, depth - 1));
  const handleDrop = (event: React.DragEvent) => {
    event.preventDefault();
    setDragDepth(0);
    const file = event.dataTransfer.files[0];
    if (file) library.importFile(file);
  };

  return (
    <div className={styles.page} onDragEnter={handleDragEnter} onDragOver={handleDragOver} onDragLeave={handleDragLeave} onDrop={handleDrop}>
      <header className={styles.header}>
        <h1 className={styles.brand}>Lectrice</h1>
        <span className={styles.tagline}>Reads PDFs aloud, locally</span>
      </header>
      <DropZone importState={library.importState} isDragging={dragDepth > 0} maxUploadMb={MAX_UPLOAD_MB} onFileChosen={library.importFile} onChooseAnother={library.resetImport} />
      <div className={styles.recent}><RecentSection library={library} /></div>
    </div>
  );
};

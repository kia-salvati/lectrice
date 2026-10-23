import { useEffect } from 'react';
import { useHashRoute } from './hooks/useHashRoute';
import { LibraryPage } from './pages/LibraryPage/LibraryPage';
import { ReaderPage } from './pages/ReaderPage/ReaderPage';

export const App = () => {
  const { route, navigate } = useHashRoute();

  // A file dropped outside the drop zone must not make the browser navigate to the PDF.
  useEffect(() => {
    const prevent = (event: DragEvent) => event.preventDefault();
    window.addEventListener('dragover', prevent);
    window.addEventListener('drop', prevent);
    return () => {
      window.removeEventListener('dragover', prevent);
      window.removeEventListener('drop', prevent);
    };
  }, []);

  if (route.name === 'reader') {
    return <ReaderPage key={route.documentId} documentId={route.documentId} onBack={() => navigate({ name: 'library' })} />;
  }
  return <LibraryPage onOpenDocument={(documentId) => navigate({ name: 'reader', documentId })} />;
};

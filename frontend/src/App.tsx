import { Suspense, lazy, useEffect } from 'react';
import { useHashRoute } from './hooks/useHashRoute';
import { LibraryPage } from './pages/LibraryPage/LibraryPage';
import { ReaderPage } from './pages/ReaderPage/ReaderPage';
import { platform } from './platform';

const desktopShell = platform.desktop;
/** Fetched only in the desktop build: the web build has no shell, so it never loads the desktop code. */
const DesktopApp = desktopShell && lazy(() => desktopShell.loadUi().then((ui) => ({ default: ui.DesktopApp })));

const WebApp = () => {
  const { route, navigate } = useHashRoute();
  if (route.name === 'reader') {
    return <ReaderPage key={route.documentId} documentId={route.documentId} onBack={() => navigate({ name: 'library' })} />;
  }
  return <LibraryPage onOpenDocument={(documentId) => navigate({ name: 'reader', documentId })} />;
};

export const App = () => {
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

  if (DesktopApp && desktopShell) {
    return <Suspense fallback={null}><DesktopApp commands={desktopShell.commands} /></Suspense>;
  }
  return <WebApp />;
};

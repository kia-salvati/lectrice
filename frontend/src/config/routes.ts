import type { DocumentId } from '../types/document';

export type Route = { name: 'library' } | { name: 'reader'; documentId: DocumentId };

const LIBRARY_HASH = '#/';
const READER_PREFIX = '#/read/';

const LIBRARY: Route = { name: 'library' };

/** '#/read/<id>' opens the reader; anything else (including a malformed id) is the library. */
export const parseRoute = (hash: string): Route => {
  if (!hash.startsWith(READER_PREFIX)) return LIBRARY;
  try {
    const documentId = decodeURIComponent(hash.slice(READER_PREFIX.length));
    return documentId ? { name: 'reader', documentId } : LIBRARY;
  } catch (error) {
    console.warn('Ignoring malformed route', hash, error);
    return LIBRARY;
  }
};

export const routeHref = (route: Route): string => (route.name === 'reader' ? `${READER_PREFIX}${encodeURIComponent(route.documentId)}` : LIBRARY_HASH);

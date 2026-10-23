import type { DocumentSummary, OpenedDocument, Sentence } from '../../types/document';
import { buildChapteredBody, buildDocument, lastPage } from './buildDocument';
import { CHAPTER, FIRST_ARABIC_PAGE, LONG_BOOK_CHAPTERS, LONG_BOOK_SUBSECTIONS, PREFACE } from './sampleText';

const LONG_BOOK_BODY_REPEATS = 2;
const LONG_BOOK_RESUME_SECTION = '7.3 ';
const LONG_BOOK_RESUME_OFFSET = 4;
const HOUR_MS = 3_600_000;
const DAY_MS = 24 * HOUR_MS;

const hoursAgo = (hours: number): string => new Date(Date.now() - hours * HOUR_MS).toISOString();

/** Every stored sentence, including skipped blocks' sentences and cues (ids are sequential over all of them). */
const allSentences = (document: OpenedDocument): Sentence[] => document.blocks.flatMap((block) => ('cue' in block && block.cue ? [block.cue, ...block.sentences] : block.sentences));

const countSentences = (document: OpenedDocument): number => allSentences(document).length;

const summarize = (document: OpenedDocument, addedAt: string, lastOpenedAt: string): DocumentSummary => ({
  id: document.id,
  title: document.title,
  sourceFileName: document.sourceFileName,
  pageCount: document.pageCount,
  sentenceCount: countSentences(document),
  addedAt,
  lastOpenedAt,
  lastSentenceId: document.lastSentenceId,
});

const calm = buildDocument({ id: 'mock-calm', title: 'Designing Calm Software', sourceFileName: 'designing-calm-software.pdf', pageCount: 18, firstArabicPage: FIRST_ARABIC_PAGE, lastSentenceId: 23 }, [...PREFACE, ...CHAPTER]);

const longBookBlocks = [...PREFACE, ...buildChapteredBody(CHAPTER.slice(1), LONG_BOOK_CHAPTERS, LONG_BOOK_SUBSECTIONS, FIRST_ARABIC_PAGE, LONG_BOOK_BODY_REPEATS)];
const longBook = buildDocument({ id: 'mock-big', title: 'The Long Book (15 chapters, about 7,000 sentences)', sourceFileName: 'the-long-book.pdf', pageCount: lastPage(longBookBlocks), firstArabicPage: FIRST_ARABIC_PAGE, lastSentenceId: null }, longBookBlocks);
/** "Continue" lands mid-book, a few sentences into chapter 7, section 3. */
const longBookResume = longBook.toc.find((entry) => entry.title.startsWith(LONG_BOOK_RESUME_SECTION));
const big: OpenedDocument = { ...longBook, lastSentenceId: longBookResume ? longBookResume.sentenceId + LONG_BOOK_RESUME_OFFSET : null };

/** No headings at all, so its `toc` is empty: the chapters panel's empty state. */
const flat = buildDocument({ id: 'mock-flat', title: 'Meeting Notes (no chapters)', sourceFileName: 'meeting-notes.pdf', pageCount: 18, firstArabicPage: FIRST_ARABIC_PAGE, lastSentenceId: null }, CHAPTER.filter((item) => item.kind !== 'heading'));

/** In-memory library. Lost on reload, which is fine for a mock. */
export const documents = new Map<string, OpenedDocument>([[calm.id, calm], [big.id, big], [flat.id, flat]]);

export const summaries: DocumentSummary[] = [
  summarize(calm, new Date(Date.now() - 12 * DAY_MS).toISOString(), hoursAgo(2)),
  summarize(big, new Date(Date.now() - 3 * DAY_MS).toISOString(), hoursAgo(26)),
  summarize(flat, new Date(Date.now() - 5 * DAY_MS).toISOString(), hoursAgo(50)),
  // Listed but not stored: opening it exercises the DocumentNotFound path.
  { id: 'mock-missing', title: 'Quarterly Notes', sourceFileName: 'quarterly-notes.pdf', pageCount: 42, sentenceCount: 900, addedAt: new Date(Date.now() - 40 * DAY_MS).toISOString(), lastOpenedAt: new Date(Date.now() - 9 * DAY_MS).toISOString(), lastSentenceId: 310 },
];

/** A newly "imported" file gets the sample content under its own id and title. */
export const addImported = (file: File): { summary: DocumentSummary; alreadyInLibrary: boolean } => {
  const id = `mock-import-${file.name}-${file.size}`;
  const now = new Date().toISOString();
  const existing = summaries.find((summary) => summary.id === id);
  if (existing) {
    existing.lastOpenedAt = now;
    return { summary: existing, alreadyInLibrary: true };
  }
  const title = file.name.replace(/\.pdf$/i, '').replace(/[-_]+/g, ' ').trim() || 'Untitled';
  const document: OpenedDocument = { ...calm, id, title, sourceFileName: file.name, lastSentenceId: null };
  documents.set(id, document);
  const summary = summarize(document, now, now);
  summaries.unshift(summary);
  return { summary, alreadyInLibrary: false };
};

export const findSentenceText = (documentId: string, sentenceId: number): string | null => {
  const document = documents.get(documentId);
  if (!document) return null;
  return allSentences(document).find((sentence) => sentence.id === sentenceId)?.text ?? null;
};

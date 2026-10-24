/** SHA-256 of the PDF bytes, hex. */
export type DocumentId = string;

/** Maps one PDF page (1-based) to the label printed on it ("xii", "37"). */
export interface PageLabel { index: number; label: string; }

/**
 * `speechText` is what the server speaks when it differs from `text` (left out when they're the same).
 * `""` means there is nothing to say (a URL-only line, a bibliography entry): shown, but playback skips it.
 */
export interface Sentence { id: number; text: string; pageIndex: number; speechText?: string; }

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

interface BlockBase { id: number; pageIndex: number; sentences: Sentence[]; }

export interface HeadingBlock extends BlockBase { kind: 'heading'; level: HeadingLevel; }
export interface ParagraphBlock extends BlockBase { kind: 'paragraph'; }
export interface ListItemBlock extends BlockBase { kind: 'listItem'; }
export interface NoteBlock extends BlockBase { kind: 'note'; }

/**
 * Shown, but skipped by playback unless "Read code & tables aloud" is on.
 * `text` keeps the original layout (line breaks) for display; `sentences` are what gets read when the switch is on.
 * `cue` (code and tables only) is a short spoken stand-in ("Code example."), never displayed.
 */
export interface SkippedBlock extends BlockBase { kind: 'code' | 'table' | 'caption'; spoken: false; text: string; cue?: Sentence; }

export type SpokenBlock = HeadingBlock | ParagraphBlock | ListItemBlock | NoteBlock;
export type Block = SpokenBlock | SkippedBlock;

/** 1 = the book's top level (a part or a chapter), 2 = sub-chapter, 3 = below that. Never deeper. */
export type TocLevel = 1 | 2 | 3;

/**
 * One table-of-contents line (spec §5). The list is flat and in reading order: `sentenceId` strictly increases,
 * and a level is at most one deeper than the entry before. `sentenceId` is the part's first sentence (jump target);
 * `pageIndex` is that sentence's PDF page (show its printed label from `pages`).
 */
export interface TocEntry { id: number; title: string; level: TocLevel; sentenceId: number; pageIndex: number; }

/** The saved, cleaned document (spec §5). Named CleanDocument so it never shadows the DOM's global `Document` type. */
export interface CleanDocument {
  id: DocumentId;
  title: string;
  sourceFileName: string;
  pageCount: number;
  cleanerVersion: number;
  pages: PageLabel[];
  /** Empty when the PDF has neither bookmarks nor detected headings. */
  toc: TocEntry[];
  blocks: Block[];
}

/** GET /api/documents/{id}: the §5 document plus where the reader stopped (spec §7), used for "Continue". */
export interface OpenedDocument extends CleanDocument { lastSentenceId: number | null; }

export interface DocumentSummary {
  id: DocumentId;
  title: string;
  sourceFileName: string;
  pageCount: number;
  sentenceCount: number;
  /** Older than the server's cleaner: opening the document cleans it again first. */
  cleanerVersion: number;
  /** ISO 8601 timestamps in UTC: "2026-10-21T18:04:00Z". */
  addedAt: string;
  lastOpenedAt: string;
  lastSentenceId: number | null;
}

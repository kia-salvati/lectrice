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

/** The saved, cleaned document (spec §5). Named CleanDocument so it never shadows the DOM's global `Document` type. */
export interface CleanDocument {
  id: DocumentId;
  title: string;
  sourceFileName: string;
  pageCount: number;
  cleanerVersion: number;
  pages: PageLabel[];
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
  /** ISO 8601 timestamps in UTC: "2026-10-21T18:04:00Z". */
  addedAt: string;
  lastOpenedAt: string;
  lastSentenceId: number | null;
}

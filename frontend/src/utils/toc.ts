import type { TocEntry } from '../types/document';

/** One entry of the flat `toc` placed in its tree. `index` is its position in the flat list; `parentIndex` null at the top. */
export interface TocNode { entry: TocEntry; index: number; parentIndex: number | null; children: TocNode[]; }

export type EntryProgress = 'read' | 'current' | 'unread';

/** A title's own number and the rest: "Chapter 4 Structured Programming" → { number: "4", name: "Structured Programming" }. */
export interface ChapterLabel { number: string | null; name: string; }

/** "Chapter 4 …", "Chapter 2. …", "Ch. IV …": the word, a number (digits or roman), optional "." or ":". */
const CHAPTER_WORD_RE = /^(?:chapter|ch\.)\s+(\d+|[ivxlc]+)\b[.:]?\s*(.*)$/i;
/** "4: Working with Pods", "1. Introduction". "20 Popular …" (no mark) is not a chapter. */
const NUMBERED_TITLE_RE = /^(\d+)[.:]\s+(.*)$/;

export const chapterLabel = (title: string): ChapterLabel => {
  const match = CHAPTER_WORD_RE.exec(title) ?? NUMBERED_TITLE_RE.exec(title);
  return match ? { number: match[1], name: match[2] } : { number: null, name: title };
};

const isChapterLike = (title: string): boolean => chapterLabel(title).number !== null;

/** "38% of this chapter" */
export const formatChapterPercent = (percent: number): string => `${percent}% of this chapter`;

/** Flat list → forest, nesting each entry under the nearest earlier entry with a smaller level. */
export const buildTocTree = (toc: readonly TocEntry[]): TocNode[] => {
  const roots: TocNode[] = [];
  const open: TocNode[] = [];
  toc.forEach((entry, index) => {
    while (open.length > 0 && open[open.length - 1].entry.level >= entry.level) open.pop();
    const parent = open[open.length - 1] ?? null;
    const node: TocNode = { entry, index, parentIndex: parent === null ? null : parent.index, children: [] };
    if (parent === null) roots.push(node);
    else parent.children.push(node);
    open.push(node);
  });
  return roots;
};

/** Every node of the forest in reading order, so `index` doubles as a lookup key. */
export const flattenTocTree = (roots: readonly TocNode[]): TocNode[] => roots.flatMap((node) => [node, ...flattenTocTree(node.children)]);

/** Index of the last entry starting at or before the sentence (any level); null before the first entry. Binary search. */
export const currentEntryIndex = (toc: readonly TocEntry[], sentenceId: number | null): number | null => {
  if (sentenceId === null) return null;
  let low = 0;
  let high = toc.length - 1;
  let found: number | null = null;
  while (low <= high) {
    const mid = (low + high) >> 1;
    if (toc[mid].sentenceId <= sentenceId) {
      found = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return found;
};

/** The entry and its ancestors as indexes, ancestors first: walking back, each earlier entry with a smaller level than the last one taken. */
export const tocPath = (toc: readonly TocEntry[], index: number): number[] => {
  const path: number[] = [];
  let level = Number.POSITIVE_INFINITY;
  for (let k = index; k >= 0; k -= 1) {
    if (toc[k].level < level) {
      path.unshift(k);
      level = toc[k].level;
    }
  }
  return path;
};

/** Where the entry's part ends: the first sentence of the next entry at the same or a higher level, or null at the end of the book. */
export const entryEndSentenceId = (toc: readonly TocEntry[], index: number): number | null => {
  const level = toc[index].level;
  for (let k = index + 1; k < toc.length; k += 1) {
    if (toc[k].level <= level) return toc[k].sentenceId;
  }
  return null;
};

/** Where a sentence is in the book, as indexes into `toc`. The last of `path` is the current entry. */
export interface TocLocation { path: number[]; chapterIndex: number; sectionIndex: number | null; }

/**
 * The chapter is the first entry on the path whose title looks like a chapter, else the path's top entry (a part, a
 * preface, a flat book's post). The section is the path entry right after it, if any. Null before the first entry.
 */
export const tocLocation = (toc: readonly TocEntry[], sentenceId: number | null): TocLocation | null => {
  const index = currentEntryIndex(toc, sentenceId);
  if (index === null) return null;
  const path = tocPath(toc, index);
  const at = Math.max(0, path.findIndex((k) => isChapterLike(toc[k].title)));
  return { path, chapterIndex: path[at], sectionIndex: path[at + 1] ?? null };
};

export const entryProgress = (toc: readonly TocEntry[], index: number, sentenceId: number | null): EntryProgress => {
  if (sentenceId === null || sentenceId < toc[index].sentenceId) return 'unread';
  const end = entryEndSentenceId(toc, index);
  return end !== null && sentenceId >= end ? 'read' : 'current';
};

/** How far into the entry's part the sentence is, 0–100, by sentence ids. */
export const percentThrough = (toc: readonly TocEntry[], index: number, sentenceId: number, sentenceCount: number): number => {
  const start = toc[index].sentenceId;
  const end = entryEndSentenceId(toc, index) ?? sentenceCount;
  if (end <= start) return 100;
  return Math.max(0, Math.min(100, Math.round(((sentenceId - start) / (end - start)) * 100)));
};

/** Sentences in the entry's part (for length-proportional bars). */
export const entryLength = (toc: readonly TocEntry[], index: number, sentenceCount: number): number => (entryEndSentenceId(toc, index) ?? sentenceCount) - toc[index].sentenceId;

/** A part groups chapters: a node with a chapter-like child. Drawn as a group heading, never as a chapter. */
export const isPartNode = (node: TocNode): boolean => node.children.some((child) => isChapterLike(child.entry.title));

/** What gets a length and progress bar: a chapter-like title, or a top-level entry that is not a part (a preface, a flat book's post). */
export const isChapterNode = (node: TocNode): boolean => !isPartNode(node) && (isChapterLike(node.entry.title) || node.parentIndex === null);

/** Everything a panel row shows about one node for the sentence being read. */
export interface NodeView {
  progress: EntryProgress;
  /** The current entry itself (the deepest one on the path). */
  isHere: boolean;
  isPart: boolean;
  /** How far into the current chapter the reader is; null on every other row. */
  percent: number | null;
  /** The read-fill of a chapter's bar: 100 once read, the percent while current, 0 before; null on non-chapter rows. */
  fillPercent: number | null;
}

export const nodeView = (toc: readonly TocEntry[], node: TocNode, location: TocLocation | null, sentenceId: number | null, sentenceCount: number): NodeView => {
  const progress = entryProgress(toc, node.index, sentenceId);
  const isPart = isPartNode(node);
  const isChapter = isChapterNode(node);
  const percent = isChapter && progress === 'current' && sentenceId !== null ? percentThrough(toc, node.index, sentenceId, sentenceCount) : null;
  const fillPercent = isChapter ? (progress === 'read' ? 100 : (percent ?? 0)) : null;
  const isHere = location !== null && location.path[location.path.length - 1] === node.index;
  return { progress, isHere, isPart, percent, fillPercent };
};

/** "6 sections", "1 chapter": what a node's children are called. */
export const childrenText = (node: TocNode): string => {
  const kind = isChapterLike(node.children[0].entry.title) ? 'chapter' : 'section';
  return `${node.children.length} ${kind}${node.children.length === 1 ? '' : 's'}`;
};

/**
 * The forest narrowed to the titles containing `query` (case-insensitive) and their ancestors; a matching node keeps
 * all its children. An empty query returns the forest unchanged.
 */
export const filterTocTree = (roots: readonly TocNode[], query: string): TocNode[] => {
  const needle = query.trim().toLowerCase();
  if (needle === '') return [...roots];
  const keep = (node: TocNode): TocNode | null => {
    if (node.entry.title.toLowerCase().includes(needle)) return node;
    const children = node.children.map(keep).filter((child): child is TocNode => child !== null);
    return children.length > 0 ? { ...node, children } : null;
  };
  return roots.map(keep).filter((node): node is TocNode => node !== null);
};

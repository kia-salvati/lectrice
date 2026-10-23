import type { Block, HeadingLevel, OpenedDocument, PageLabel, Sentence, TocEntry, TocLevel } from '../../types/document';
import type { SourceBlock } from './sampleText';

interface DocumentMeta { id: string; title: string; sourceFileName: string; pageCount: number; firstArabicPage: number; lastSentenceId: number | null; }

const ROMAN: ReadonlyArray<[number, string]> = [[1000, 'm'], [900, 'cm'], [500, 'd'], [400, 'cd'], [100, 'c'], [90, 'xc'], [50, 'l'], [40, 'xl'], [10, 'x'], [9, 'ix'], [5, 'v'], [4, 'iv'], [1, 'i']];

const toRoman = (value: number): string => {
  let rest = value;
  let result = '';
  for (const [amount, letters] of ROMAN) {
    while (rest >= amount) {
      result += letters;
      rest -= amount;
    }
  }
  return result;
};

/** Front matter gets roman labels, the body restarts at "1": PDF page 14 → "1". */
const buildPageLabels = (pageCount: number, firstArabicPage: number): PageLabel[] => Array.from({ length: pageCount }, (_, i) => {
  const index = i + 1;
  return { index, label: index < firstArabicPage ? toRoman(index) : String(index - firstArabicPage + 1) };
});

const isTocLevel = (level: HeadingLevel): level is TocLevel => level <= 3;

/** What the server builds when a PDF has no bookmarks: every heading down to level 3, in reading order. */
const buildToc = (blocks: readonly Block[]): TocEntry[] => blocks
  .flatMap((block) => (block.kind === 'heading' && isTocLevel(block.level)
    ? [{ title: block.sentences.map((sentence) => sentence.text).join(' '), level: block.level, sentenceId: block.sentences[0].id, pageIndex: block.pageIndex }]
    : []))
  .map((entry, id) => ({ id, ...entry }));

/** Turns compact source blocks into the spec §5 shape with global, sequential block and sentence ids. */
export const buildDocument = (meta: DocumentMeta, source: readonly SourceBlock[]): OpenedDocument => {
  let nextSentenceId = 0;
  const sentence = (text: string, pageIndex: number): Sentence => ({ id: nextSentenceId++, text, pageIndex });
  const blocks: Block[] = source.map((item, blockId): Block => {
    if (item.kind === 'code') {
      const cue = item.cue === undefined ? undefined : sentence(item.cue, item.page);
      const sentences = item.text.split('\n').map((line) => line.trim()).filter(Boolean).map((line) => sentence(line, item.page));
      return { id: blockId, kind: 'code', spoken: false, pageIndex: item.page, text: item.text, sentences, ...(cue ? { cue } : {}) };
    }
    if (item.kind === 'caption') {
      const sentences = item.sentences.map((text) => sentence(text, item.page));
      return { id: blockId, kind: 'caption', spoken: false, pageIndex: item.page, text: item.sentences.join(' '), sentences };
    }
    if (item.kind === 'heading') return { id: blockId, kind: 'heading', level: item.level, pageIndex: item.page, sentences: [sentence(item.text, item.page)] };
    const sentences = item.sentences.map((text, i) => sentence(text, item.nextPageFrom !== undefined && i >= item.nextPageFrom ? item.page + 1 : item.page));
    return { id: blockId, kind: item.kind, pageIndex: item.page, sentences };
  });
  return {
    id: meta.id,
    title: meta.title,
    sourceFileName: meta.sourceFileName,
    pageCount: meta.pageCount,
    cleanerVersion: 1,
    pages: buildPageLabels(meta.pageCount, meta.firstArabicPage),
    toc: buildToc(blocks),
    blocks,
    lastSentenceId: meta.lastSentenceId,
  };
};

export interface ChapterPlan { title: string; sections: readonly string[]; }

const ONE_LEVEL_DEEPER: Record<HeadingLevel, HeadingLevel> = { 1: 2, 2: 3, 3: 4, 4: 5, 5: 6, 6: 6 };

/** The body moved to start on `page`, its headings one level deeper and named by `headingTitle(n)` (n counts them). */
const placeBody = (body: readonly SourceBlock[], bodyStart: number, page: number, headingTitle: ((n: number) => string) | null): SourceBlock[] => {
  let headings = 0;
  return body
    .filter((item) => headingTitle !== null || item.kind !== 'heading')
    .map((item): SourceBlock => {
      const shifted = page + item.page - bodyStart;
      if (item.kind === 'heading' && headingTitle) return { ...item, page: shifted, level: ONE_LEVEL_DEEPER[item.level], text: headingTitle(headings++) };
      return { ...item, page: shifted };
    });
};

/**
 * A long book out of one sample body: each planned chapter gets a level-1 heading ("7. Title"), each section a level-2
 * heading ("7.3 Title") followed by `bodyRepeats` copies of the body. Only the first copy keeps its headings, one level
 * deeper and renamed from `subsectionTitles` (rotated per section so neighbours differ).
 */
export const buildChapteredBody = (body: readonly SourceBlock[], chapters: readonly ChapterPlan[], subsectionTitles: readonly string[], firstPage: number, bodyRepeats: number): SourceBlock[] => {
  const bodyStart = Math.min(...body.map((item) => item.page));
  const bodyPages = Math.max(...body.map((item) => item.page)) - bodyStart + 1;
  const blocks: SourceBlock[] = [];
  let page = firstPage;
  let sectionCount = 0;
  chapters.forEach((chapter, chapterIndex) => {
    blocks.push({ kind: 'heading', level: 1, page, text: `${chapterIndex + 1}. ${chapter.title}` });
    chapter.sections.forEach((section, sectionIndex) => {
      blocks.push({ kind: 'heading', level: 2, page, text: `${chapterIndex + 1}.${sectionIndex + 1} ${section}` });
      const rotation = sectionCount++;
      for (let copy = 0; copy < bodyRepeats; copy++) {
        const headingTitle = copy === 0 ? (n: number) => subsectionTitles[(rotation + n) % subsectionTitles.length] : null;
        blocks.push(...placeBody(body, bodyStart, page, headingTitle));
        page += bodyPages;
      }
    });
  });
  return blocks;
};

/** The last PDF page any block is on, for `pageCount`. */
export const lastPage = (blocks: readonly SourceBlock[]): number => Math.max(...blocks.map((item) => item.page));

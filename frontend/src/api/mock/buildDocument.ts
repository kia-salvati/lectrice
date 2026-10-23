import type { Block, OpenedDocument, PageLabel, Sentence } from '../../types/document';
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
    blocks,
    lastSentenceId: meta.lastSentenceId,
  };
};

/** Shifts a chapter by whole pages and renames its first heading, to build a long document out of one chapter. */
export const repeatChapter = (chapter: readonly SourceBlock[], times: number, pagesPerChapter: number): SourceBlock[] => Array.from({ length: times }, (_, round) => chapter.map((item, i): SourceBlock => {
  const page = item.page + round * pagesPerChapter;
  if (i === 0 && item.kind === 'heading') return { ...item, page, text: `${round + 1}. ${item.text.replace(/^\d+\.\s*/, '')}` };
  return { ...item, page };
})).flat();

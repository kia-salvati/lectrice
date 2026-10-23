import type { CleanDocument, PageLabel } from '../types/document';

/** PDF page index → printed label. A page without a label falls back to its PDF number. */
export const buildPageLabelLookup = (pages: readonly PageLabel[]): Map<number, string> => new Map(pages.map((page) => [page.index, page.label]));

export const printedLabel = (labels: ReadonlyMap<number, string>, pageIndex: number): string => labels.get(pageIndex) ?? String(pageIndex);

/** "p. 37" */
export const formatPageMarker = (label: string): string => `p. ${label}`;

/** "PDF page 61" */
export const formatPdfPage = (pageIndex: number): string => `PDF page ${pageIndex}`;

/**
 * Where page markers go. Code and table blocks are drawn as one box, so their marker goes before the block;
 * everything else is drawn sentence by sentence, so the marker goes before the sentence where the page changes.
 * The first content always gets one.
 */
export interface PageMarkerPlan { beforeSentence: Set<number>; beforeBlock: Set<number>; }

export const planPageMarkers = (document: CleanDocument): PageMarkerPlan => {
  const plan: PageMarkerPlan = { beforeSentence: new Set(), beforeBlock: new Set() };
  let previousPage: number | null = null;
  for (const block of document.blocks) {
    if (block.kind === 'code' || block.kind === 'table') {
      if (block.pageIndex !== previousPage) plan.beforeBlock.add(block.id);
      previousPage = block.pageIndex;
      continue;
    }
    for (const sentence of block.sentences) {
      if (sentence.pageIndex !== previousPage) plan.beforeSentence.add(sentence.id);
      previousPage = sentence.pageIndex;
    }
  }
  return plan;
};

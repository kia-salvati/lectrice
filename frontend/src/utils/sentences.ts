import type { CleanDocument } from '../types/document';

/** One past the highest sentence id, cues included (ids are sequential from 0): where the book's last part ends. */
export const sentenceIdEnd = (document: CleanDocument): number => document.blocks.reduce((end, block) => {
  const cueEnd = 'cue' in block && block.cue ? block.cue.id + 1 : 0;
  return Math.max(end, cueEnd, ...block.sentences.map((sentence) => sentence.id + 1));
}, 0);

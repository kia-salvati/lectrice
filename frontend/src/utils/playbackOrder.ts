import type { Block, CleanDocument, Sentence } from '../types/document';

/**
 * What playback reads, in order, for one setting of "Read code & tables aloud". Built once per (document, readSkipped).
 * Rule for a skipped block (code, table, caption): readSkipped off → its cue if it has one, else nothing;
 * readSkipped on → its own sentences (never the cue). Every other block → its sentences.
 * Silent sentences (`speechText: ""`) are left out; they stay on screen as normal text.
 */
export interface PlaybackOrder {
  sentenceIds: number[];
  /** Sentence id → position in sentenceIds. */
  positionOf: Map<number, number>;
  /** Block index (position in document.blocks) of each position in sentenceIds. */
  blockIndexAt: number[];
  /** charsFrom[i] = characters from sentenceIds[i] to the end. Has one extra trailing 0. */
  charsFrom: number[];
  /** Sentence id → PDF page, for every sentence in the order. */
  pageIndexOf: Map<number, number>;
  /** Block index of every sentence in the document, cues included, so any id can be placed. */
  blockIndexOfAny: Map<number, number>;
  /** Silent sentence id (in a block that is read) → position of the next playable sentence, which may be past the end. */
  nextPositionOfSilent: Map<number, number>;
}

/** The sentences a block reads for this setting, silent ones included. */
const readSentences = (block: Block, readSkipped: boolean): Sentence[] => {
  if (!('spoken' in block)) return block.sentences;
  if (readSkipped) return block.sentences;
  return block.cue ? [block.cue] : [];
};

/** The server sends `speechText: ""` for sentences with nothing to say; playing them would only be silence. */
const isSilent = (sentence: Sentence): boolean => sentence.speechText === '';

const spokenLength = (sentence: Sentence): number => (sentence.speechText ?? sentence.text).length;

export const buildPlaybackOrder = (document: CleanDocument, readSkipped: boolean): PlaybackOrder => {
  const sentenceIds: number[] = [];
  const blockIndexAt: number[] = [];
  const lengths: number[] = [];
  const pageIndexOf = new Map<number, number>();
  const blockIndexOfAny = new Map<number, number>();
  const nextPositionOfSilent = new Map<number, number>();
  document.blocks.forEach((block, blockIndex) => {
    for (const sentence of block.sentences) blockIndexOfAny.set(sentence.id, blockIndex);
    if ('cue' in block && block.cue) blockIndexOfAny.set(block.cue.id, blockIndex);
    for (const sentence of readSentences(block, readSkipped)) {
      if (isSilent(sentence)) {
        nextPositionOfSilent.set(sentence.id, sentenceIds.length);
        continue;
      }
      sentenceIds.push(sentence.id);
      blockIndexAt.push(blockIndex);
      lengths.push(spokenLength(sentence));
      pageIndexOf.set(sentence.id, sentence.pageIndex);
    }
  });
  const charsFrom = new Array<number>(lengths.length + 1).fill(0);
  for (let i = lengths.length - 1; i >= 0; i -= 1) charsFrom[i] = charsFrom[i + 1] + lengths[i];
  const positionOf = new Map(sentenceIds.map((id, position) => [id, position]));
  return { sentenceIds, positionOf, blockIndexAt, charsFrom, pageIndexOf, blockIndexOfAny, nextPositionOfSilent };
};

/** First playable sentence in this block or any later one, or null past the end. */
export const firstPlayableFromBlock = (order: PlaybackOrder, blockIndex: number): number | null => {
  const position = order.blockIndexAt.findIndex((at) => at >= blockIndex);
  return position === -1 ? null : order.sentenceIds[position];
};

/**
 * Maps any sentence id onto this order: itself if playable, the next playable one if it is silent,
 * otherwise the first playable sentence from its block on.
 * Used for saved positions, clicks on skipped or silent text, and when the readSkipped switch changes mid-book.
 */
export const resolvePlayable = (order: PlaybackOrder, sentenceId: number): number | null => {
  if (order.positionOf.has(sentenceId)) return sentenceId;
  const nextPosition = order.nextPositionOfSilent.get(sentenceId);
  if (nextPosition !== undefined) return order.sentenceIds[nextPosition] ?? null;
  const blockIndex = order.blockIndexOfAny.get(sentenceId);
  return blockIndex === undefined ? null : firstPlayableFromBlock(order, blockIndex);
};

/** Where to start: the saved sentence (resolved), otherwise the first playable one. Null if nothing is playable. */
export const startingSentenceId = (order: PlaybackOrder, lastSentenceId: number | null): number | null => {
  const resolved = lastSentenceId === null ? null : resolvePlayable(order, lastSentenceId);
  return resolved ?? order.sentenceIds[0] ?? null;
};

/** The playable sentence `offset` steps away (negative = back), or null past either end. */
export const stepFrom = (order: PlaybackOrder, sentenceId: number, offset: number): number | null => {
  const position = order.positionOf.get(sentenceId);
  if (position === undefined) return null;
  return order.sentenceIds[position + offset] ?? null;
};

/** The current sentence and the next `ahead` playable ones. */
export const prefetchWindow = (order: PlaybackOrder, sentenceId: number, ahead: number): number[] => {
  const position = order.positionOf.get(sentenceId);
  if (position === undefined) return [];
  return order.sentenceIds.slice(position, position + ahead + 1);
};

export const secondsLeft = (order: PlaybackOrder, sentenceId: number, speed: number, secondsPerChar: number): number => {
  const position = order.positionOf.get(sentenceId) ?? 0;
  return (order.charsFrom[position] * secondsPerChar) / speed;
};

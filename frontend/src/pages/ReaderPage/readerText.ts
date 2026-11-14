import { SECONDS_PER_CHAR } from '../../config/constants';
import type { TocEntry } from '../../types/document';
import { formatCount } from '../../utils/numbers';
import { formatPageMarker, formatPdfPage } from '../../utils/pages';
import { secondsBetween } from '../../utils/playbackOrder';
import type { PlaybackOrder } from '../../utils/playbackOrder';
import { playerStatusParts } from '../../utils/playerStatus';
import type { PlayerState, StatusPart } from '../../utils/playerStatus';
import { formatTimeLeft } from '../../utils/time';
import { chapterLabel, entryEndSentenceId, formatChapterPercent, percentThrough } from '../../utils/toc';
import type { TocLocation } from '../../utils/toc';

interface StatusInput {
  state: PlayerState;
  /** The shell's own words for a missing voice; they replace "Speech engine unavailable". */
  voiceStatus: string | null;
  pageLabel: string;
  lastPageLabel: string;
  chapterTitle: string | null;
  speed: number;
}

/** The pill's status parts. */
export const statusParts = ({ state, voiceStatus, pageLabel, lastPageLabel, chapterTitle, speed }: StatusInput): StatusPart[] => (state === 'engineFailed' && voiceStatus !== null
  ? [{ text: voiceStatus, shrinks: true }]
  : playerStatusParts(state, { pageText: formatPageMarker(pageLabel), lastPageText: lastPageLabel, chapter: chapterTitle === null ? null : chapterLabel(chapterTitle) }, speed));

/** "Sentence 120 of 7,939, page 37": what the progress slider says. */
export const positionText = (position: number, playableCount: number, pageLabel: string): string => `Sentence ${formatCount(position + 1)} of ${formatCount(playableCount)}, page ${pageLabel}`;

/** "Page 37 · PDF page 61 of 429" */
export const locationText = (pageLabel: string, pageIndex: number | null, pageCount: number): string => `Page ${pageLabel} · ${pageIndex === null ? '' : formatPdfPage(pageIndex)} of ${formatCount(pageCount)}`;

interface ChapterLineInput {
  toc: readonly TocEntry[];
  location: TocLocation;
  sentenceId: number;
  sentenceCount: number;
  order: PlaybackOrder;
  speed: number;
}

/** The popover's chapter line: "7. Long Documents › 7.3 Tables" and "38 % of this chapter · 9 min left in it". */
export const chapterLine = ({ toc, location, sentenceId, sentenceCount, order, speed }: ChapterLineInput): { pathText: string; progressText: string } => {
  const chapterTitle = toc[location.chapterIndex].title;
  const pathText = location.sectionIndex === null ? chapterTitle : `${chapterTitle} › ${toc[location.sectionIndex].title}`;
  const percentText = formatChapterPercent(percentThrough(toc, location.chapterIndex, sentenceId, sentenceCount));
  const secondsLeftInChapter = secondsBetween(order, sentenceId, entryEndSentenceId(toc, location.chapterIndex), speed, SECONDS_PER_CHAR);
  return { pathText, progressText: `${percentText} · ${formatTimeLeft(secondsLeftInChapter)} in it` };
};

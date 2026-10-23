import type { ChapterLabel } from './toc';

export type PlaybackStatus = 'paused' | 'preparing' | 'playing' | 'ended';

/** `unreachable` means /health itself failed. */
export type EngineState = 'checking' | 'ready' | 'failed' | 'unreachable';

export type PlayerState = 'engineFailed' | 'preparing' | 'playing' | 'paused' | 'ended';

/** One span of the pill's status. Only a part that `shrinks` gets the ellipsis; the others keep their width. */
export interface StatusPart { text: string; shrinks: boolean; }

/** Where the reader is: "p. 37" / "p. 37 of 412" and the chapter, when the book has one here. */
export interface StatusLocation { pageText: string; lastPageText: string; chapter: ChapterLabel | null; }

const ENGINE_BLOCKS_PLAYBACK: Record<EngineState, boolean> = { checking: false, ready: false, failed: true, unreachable: true };

export const playerState = (engine: EngineState, status: PlaybackStatus): PlayerState => (ENGINE_BLOCKS_PLAYBACK[engine] ? 'engineFailed' : status);

/** "Ch. 7" */
const formatChapterNumber = (number: string): string => `Ch. ${number}`;

const fixed = (text: string): StatusPart => ({ text, shrinks: false });
const shrinking = (text: string): StatusPart => ({ text, shrinks: true });

const speedPart = (speed: number): StatusPart[] => (speed === 1 ? [] : [fixed(`${speed}×`)]);

/** "Ch. 7 · Long Documents · p. 213" with a chapter, else today's "p. 37 of 412" as one span. */
const locationParts = ({ pageText, lastPageText, chapter }: StatusLocation): StatusPart[] => {
  if (chapter === null) return [shrinking(`${pageText} of ${lastPageText}`)];
  const number = chapter.number === null ? [] : [fixed(formatChapterNumber(chapter.number))];
  return [...number, shrinking(chapter.name), fixed(pageText)];
};

const STATUS_PARTS: Record<PlayerState, (location: StatusLocation, speed: number) => StatusPart[]> = {
  engineFailed: () => [shrinking('Speech engine unavailable')],
  preparing: () => [shrinking('Preparing audio…')],
  playing: (location, speed) => [...locationParts(location), ...speedPart(speed)],
  paused: (location) => [fixed('Paused'), ...locationParts(location)],
  ended: () => [shrinking('Finished')],
};

/** The pill's short status, as parts joined by " · ". */
export const playerStatusParts = (state: PlayerState, location: StatusLocation, speed: number): StatusPart[] => STATUS_PARTS[state](location, speed);

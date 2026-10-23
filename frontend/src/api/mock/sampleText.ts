import type { HeadingLevel } from '../../types/document';
import type { ChapterPlan } from './buildDocument';

/**
 * Compact source for mock documents. `page` is the PDF page (1-based). `nextPageFrom: k` puts sentences k.. on page + 1.
 * A code block's sentences are its non-empty lines; `cue` is what playback says instead when skipping it.
 */
export type SourceBlock =
  | { kind: 'heading'; level: HeadingLevel; page: number; text: string }
  | { kind: 'paragraph' | 'listItem' | 'note'; page: number; sentences: string[]; nextPageFrom?: number }
  | { kind: 'code'; page: number; text: string; cue?: string }
  | { kind: 'caption'; page: number; sentences: string[] };

/** PDF pages 12–13 carry roman labels (xii, xiii); page 14 is printed "1". */
export const FIRST_ARABIC_PAGE = 14;

export const PREFACE: SourceBlock[] = [
  { kind: 'heading', level: 1, page: 12, text: 'Preface' },
  { kind: 'paragraph', page: 12, sentences: [
    'This book is about software that stays out of the way.',
    'Most of us have used tools that shout: banners that never close, badges that never clear, and settings hidden three menus deep.',
    'Calm software does the opposite.',
    'It shows what matters now, keeps everything else one step away, and never makes the reader wonder what just happened.',
    'That sounds simple, and in a sense it is.',
  ] },
  { kind: 'paragraph', page: 12, sentences: [
    'The chapters that follow are short on purpose.',
    'Each one takes a single idea, shows where it came from, and ends with something you can try on your own project the same afternoon.',
    'You do not need to read them in order.',
    'If a chapter does not fit the work in front of you, skip it and come back later.',
  ] },
  { kind: 'paragraph', page: 13, sentences: [
    'A few words about the examples.',
    'They are written in TypeScript because it is widely read, but nothing in them depends on a particular language or framework.',
    'Where an example needs a server, it assumes a small local one, the kind you might run on a laptop while you work.',
    'Three habits come up again and again:',
  ] },
  { kind: 'listItem', page: 13, sentences: ['Show one thing at a time, and make it obvious which one it is.'] },
  { kind: 'listItem', page: 13, sentences: ['Say what a control will do before it is pressed.'] },
  { kind: 'listItem', page: 13, sentences: ['When something goes wrong, say what the person can do next.'] },
  { kind: 'paragraph', page: 13, sentences: [
    'None of these are new.',
    'What is new, perhaps, is treating them as engineering rules rather than matters of taste.',
  ] },
];

export const CHAPTER: SourceBlock[] = [
  { kind: 'heading', level: 1, page: 14, text: '1. Quiet Interfaces' },
  { kind: 'paragraph', page: 14, sentences: [
    'An interface is quiet when nothing on the screen competes for attention without a reason.',
    'That does not mean empty.',
    'A quiet screen can be dense with information, as long as the information is arranged so that the eye knows where to start.',
    'The difference between a busy screen and a rich one is mostly a matter of hierarchy: one clear primary element, a few secondary ones, and everything else visibly less important.',
    'Hierarchy is cheap to add and expensive to remove, so it pays to decide it early.',
  ] },
  { kind: 'heading', level: 2, page: 14, text: '1.1 Fewer, clearer signals' },
  { kind: 'paragraph', page: 14, sentences: [
    'Every colour, icon and badge is a signal, and signals wear out.',
    'A red dot that appears for every minor update soon means nothing, and then it fails on the day it carries real news.',
    'The fix is to spend signals deliberately.',
    'Give each one a single meaning, write that meaning down, and remove any signal that has drifted from it.',
    'Teams that do this often find they can delete half of their notifications without anyone noticing, except to say that the product feels easier.',
  ] },
  { kind: 'paragraph', page: 15, nextPageFrom: 3, sentences: [
    'Signals also need to be readable.',
    'Grey text on a grey background may look elegant in a mock-up, but it fails the moment the screen is in sunlight or the reader is tired.',
    'A useful rule is to measure contrast rather than judge it by eye, and to treat any text below a ratio of four and a half to one as a bug.',
    'The same applies to size.',
    'Text that must be read should be large enough to read at arm’s length on the smallest device you support, which for most products today is a phone held in one hand on a moving train.',
    'Labels that are only decoration can be smaller, but there are fewer of those than designers tend to think.',
  ] },
  { kind: 'heading', level: 2, page: 16, text: '1.2 A small example' },
  { kind: 'paragraph', page: 16, sentences: [
    'Consider a status line that reports the state of a background job.',
    'A first version often maps each state to a colour with a chain of conditions.',
    'A calmer version keeps the mapping in one table, so that adding a state is one line and nothing can be forgotten:',
  ] },
  { kind: 'code', page: 16, text: "const STATUS_LABEL: Record<JobStatus, string> = {\n  queued: 'Waiting to start',\n  running: 'Working…',\n  failed: 'Stopped. Try again?',\n  done: 'Finished',\n};", cue: 'Code example.' },
  { kind: 'caption', page: 16, sentences: ['Listing 1.1. A status table that keeps every message in one place.'] },
  { kind: 'paragraph', page: 16, sentences: [
    'The table does more than tidy the code.',
    'It puts every message the user can see in one place, where a reviewer can read them side by side and notice that one of them is rude, vague or wrong.',
    'It also makes the compiler check that no state is missing.',
    'That is the whole idea.',
  ] },
  { kind: 'heading', level: 2, page: 17, text: '1.3 Measuring calm' },
  { kind: 'paragraph', page: 17, sentences: [
    'Calm is hard to measure directly, but its absence leaves traces.',
    'Support requests that begin with “I can’t find” or “what does this mean” point at hierarchy and wording.',
    'Repeated clicks on the same control point at missing feedback.',
    'Long pauses before a destructive action point at a label that does not say what will happen.',
    'None of these need special tools; a week of reading support mail is usually enough to find the worst of them.',
  ] },
  { kind: 'note', page: 17, sentences: ['Note: none of these measures replace talking to the people who use the product.'] },
  { kind: 'paragraph', page: 17, sentences: [
    'Once you have a list, fix the cheapest items first.',
    'A clearer label costs minutes, and a removed notification costs nothing at all.',
    'Larger changes, such as reorganising a screen, are easier to argue for once the small fixes have shown that calm is something users notice.',
    'Keep the list somewhere the whole team can see it.',
  ] },
  { kind: 'heading', level: 3, page: 17, text: 'What to try next' },
  { kind: 'paragraph', page: 17, sentences: [
    'Pick one screen you know well.',
    'Write down every signal on it, and next to each one, the single thing it is supposed to mean.',
    'Cross out the ones you cannot explain in a sentence.',
    'Then show the crossed-out list to a colleague and ask which of them they would miss.',
    'You may be surprised by how short their answer is.',
  ] },
];

/** Level-3 headings inside the Long Book's sections. */
export const LONG_BOOK_SUBSECTIONS = ['Why it matters', 'In practice', 'Common mistakes', 'A short checklist', 'What the research says', 'Try it yourself'];

/** The Long Book's outline: 15 chapters of 3–8 sections, some with long titles, to exercise the chapters panel. */
export const LONG_BOOK_CHAPTERS: ChapterPlan[] = [
  { title: 'Quiet Interfaces', sections: ['Fewer, clearer signals', 'A small example', 'Measuring calm', 'When quiet becomes hidden'] },
  { title: 'Reading on Small Screens', sections: ['Line length and the thumb', 'Type that survives sunlight', 'Scrolling versus paging', 'One-handed controls', 'Night reading', 'Summary'] },
  { title: 'Sound as an Interface', sections: ['Voices people trust', 'Pace, pauses and breath', 'What not to read aloud'] },
  { title: 'Designing for Interruptions: How People Really Read on Phones, on Trains and Late at Night in Bed', sections: ['The three-second glance', 'Finding your place again', 'Bookmarks nobody sets', 'Resuming without asking', 'Progress that means something', 'Notifications that wait their turn', 'A field study in a commuter train', 'Summary'] },
  { title: 'Navigation Without Menus', sections: ['Where am I?', 'Where can I go?', 'Breadcrumbs, rails and drawers', 'Keyboard paths', 'Summary'] },
  { title: 'Errors That Help', sections: ['Say what happened', 'Say what to do next', 'Never blame the reader', 'Recovering a lost import: a worked example with a corrupted PDF', 'Summary'] },
  { title: 'Long Documents', sections: ['Rendering only what is visible', 'Search that respects structure', 'Tables of contents that tell the truth', 'Footnotes, figures and asides', 'Index pages and why we drop them', 'Very long titles and how to shorten them', 'Summary'] },
  { title: 'Colour and Contrast', sections: ['Measuring instead of guessing', 'Dark mode is not inverted light mode', 'Accents that carry meaning', 'Summary'] },
  { title: 'Motion With a Purpose', sections: ['Transitions that explain', 'Reduced motion', 'Summary'] },
  { title: 'Settings People Actually Change', sections: ['Speed', 'Voice', 'Read code and tables aloud', 'Defaults that respect the reader', 'Remembering choices per book', 'Summary'] },
  { title: 'Offline First', sections: ['Everything on one machine', 'Caching audio ahead of the listener', 'Exports that finish in the background', 'Summary'] },
  { title: 'Accessibility Is the Baseline', sections: ['Screen readers and live regions', 'Focus that never gets lost', 'Targets big enough for every hand', 'Testing with real assistive technology', 'Summary'] },
  { title: 'Performance You Can Feel', sections: ['The first sentence in under a second', 'Prefetching without waste', 'Memory on old laptops', 'Summary'] },
  { title: 'Shipping Calm Software', sections: ['Reviews that read the copy', 'Release notes in plain words', 'Listening after launch', 'Saying no to features', 'Summary'] },
  { title: 'Afterword: What We Left Out on Purpose', sections: ['Social features', 'Gamification', 'Thanks'] },
];

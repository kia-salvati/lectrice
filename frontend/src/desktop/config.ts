import type { LinkId, ModelVariant } from './commands/types';

/** English voices in every bundle. */
export const VOICE_COUNT = 28;

/** The fixed sentence every voice sample says. */
export const SAMPLE_LINE = 'This is how I sound when I read your books aloud.';

/** The bundle the first-run screen recommends. */
export const RECOMMENDED_VARIANT: ModelVariant = 'fast';

/** The official int8 model is no longer offered: listed only while installed or partly downloaded. */
export const RETIRED_VARIANT: ModelVariant = 'int8';

export const VARIANT_NOTE: Record<ModelVariant, string> = {
  fast: 'Our own build of the same voice. Sounds the same as Full quality in our tests, and makes speech fastest.',
  fp32: 'The original voice model, the same as the Android app. The largest download.',
  small: "Our own build with smaller weights. Sounds like Full quality, at Full quality's speed.",
  int8: 'No longer offered: slower and a little less natural. Fast is better.',
};

export const FFMPEG_INSTALL_COMMANDS: readonly { label: string; command: string }[] = [
  { label: 'Fedora', command: 'sudo dnf install ffmpeg-free' },
  { label: 'Ubuntu, Debian, Mint', command: 'sudo apt install ffmpeg' },
];

/** How long "Copy" reads "Copied". */
export const COPIED_FEEDBACK_MS = 2000;

export const LINK_LABEL: Record<Extract<LinkId, 'releases' | 'source'>, string> = { releases: 'Newer versions on GitHub', source: 'Source code on GitHub' };

/** Characters of a path kept whole at its end when it is shortened in the middle. */
export const PATH_TAIL_LENGTH = 16;

export const SETTINGS_SHORTCUT_KEY = ',';

/** The pairing countdown's bar and "left" text are redrawn this often. */
export const PAIRING_TICK_MS = 250;
/** "Paired with …" stays this long before the pairing card closes by itself. */
export const PAIRED_CARD_CLOSES_MS = 2000;
/** "Last seen 5 min ago" and the cool-down minutes are counted again this often. */
export const RELATIVE_TIME_TICK_MS = 30_000;
export const COMPUTER_NAME_MAX_LENGTH = 40;

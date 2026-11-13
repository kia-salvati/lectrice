import type { LinkId, ModelImportKind, ModelVariant } from './commands/types';

/** English voices in every bundle. */
export const VOICE_COUNT = 28;

/** The fixed sentence every voice sample says. */
export const SAMPLE_LINE = 'This is how I sound when I read your books aloud.';

/** The bundle the first-run screen recommends. */
export const RECOMMENDED_VARIANT: ModelVariant = 'fast';

export const VARIANT_NOTE: Record<ModelVariant, string> = {
  fast: 'Sounds like Full quality and reads faster. Recommended.',
  fp32: 'The original model. The biggest download.',
  small: 'Sounds like Full quality, in a smaller file.',
  int8: 'No longer offered: slower and a little less natural. Fast is better.',
};

/** The archives and unpacked folders an import accepts, named in the wrong-file and wrong-folder messages. The retired int8 is still accepted but not named. */
const OFFERED_FOLDER_NAMES = ['kokoro-lectrice-q8-multi-lang-v1_0', 'kokoro-multi-lang-v1_0', 'kokoro-lectrice-w8-multi-lang-v1_0'];
const listed = (names: readonly string[]): string => `${names.slice(0, -1).join(', ')} or ${names[names.length - 1]}`;

interface ImportText {
  checking: string;
  /** The archive is unpacked; the folder's files are copied. */
  unpacking: string;
  notAModel: string;
  unreadable: string;
  pickAnother: string;
}

/** What an import's progress line, its failures and its retry button say. The shell reports both kinds as `source: 'import'`. */
export const IMPORT_TEXT: Record<ModelImportKind, ImportText> = {
  archive: {
    checking: 'Checking the file…',
    unpacking: 'Unpacking…',
    notAModel: `This file isn't a Lectrice voice model. Pick ${listed(OFFERED_FOLDER_NAMES.map((name) => `${name}.tar.bz2`))}.`,
    unreadable: "This file couldn't be opened.",
    pickAnother: 'Pick another file…',
  },
  folder: {
    checking: 'Checking the folder…',
    unpacking: 'Copying the voice files…',
    notAModel: `This folder isn't a Lectrice voice model. Pick the unpacked ${listed(OFFERED_FOLDER_NAMES)} folder.`,
    unreadable: "This folder couldn't be opened.",
    pickAnother: 'Pick another folder…',
  },
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

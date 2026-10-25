import type { LinkId, ModelImportKind, ModelVariant } from './commands/types';

/** English voices in both bundles. */
export const VOICE_COUNT = 28;

/** The fixed sentence every voice sample says. */
export const SAMPLE_LINE = 'This is how I sound when I read your books aloud.';

export const VARIANT_NOTE: Record<ModelVariant, string> = { fp32: 'Sounds the most natural.', int8: 'Saves space. A little less natural.' };

/** The archives and unpacked folders an import accepts, named in the wrong-file and wrong-folder messages. */
export const IMPORT_FOLDER_NAMES: Record<ModelVariant, string> = { fp32: 'kokoro-multi-lang-v1_0', int8: 'kokoro-int8-multi-lang-v1_0' };
export const IMPORT_FILE_NAMES: Record<ModelVariant, string> = { fp32: `${IMPORT_FOLDER_NAMES.fp32}.tar.bz2`, int8: `${IMPORT_FOLDER_NAMES.int8}.tar.bz2` };

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
    notAModel: `This file isn't a Lectrice voice model. Pick ${IMPORT_FILE_NAMES.fp32} or ${IMPORT_FILE_NAMES.int8}.`,
    unreadable: "This file couldn't be opened.",
    pickAnother: 'Pick another file…',
  },
  folder: {
    checking: 'Checking the folder…',
    unpacking: 'Copying the voice files…',
    notAModel: `This folder isn't a Lectrice voice model. Pick the unpacked ${IMPORT_FOLDER_NAMES.fp32} or ${IMPORT_FOLDER_NAMES.int8} folder.`,
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

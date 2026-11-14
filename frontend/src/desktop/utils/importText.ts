import type { ModelBundle, ModelImportKind } from '../commands/types.ts';
import { offered } from './modelSetup.ts';

/** `.tar.bz2`, `.zip`: the file type of a model archive. */
const ARCHIVE_SUFFIX = /(\.tar)?\.[a-z0-9]+$/i;

export const archiveSuffix = (archiveName: string): string => ARCHIVE_SUFFIX.exec(archiveName)?.[0] ?? '';


const listed = (names: readonly string[]): string => (names.length < 2 ? names.join('') : `${names.slice(0, -1).join(', ')} or ${names[names.length - 1]}`);

/** "Model file (.tar.bz2)…" in the import menu. */
export const importFileLabel = (bundles: readonly ModelBundle[]): string => {
  const suffix = archiveSuffix(offered(bundles)[0]?.archiveName ?? '');
  return suffix === '' ? 'Model file…' : `Model file (${suffix})…`;
};

export interface ImportText {
  checking: string;
  /** The archive is unpacked; the folder's files are copied. */
  unpacking: string;
  notAModel: string;
  unreadable: string;
  pickAnother: string;
}

/** What an import's progress line, its failures and its retry button say, naming the archives the shell offers. */
export const importText = (kind: ModelImportKind, bundles: readonly ModelBundle[]): ImportText => {
  const shown = offered(bundles);
  return kind === 'archive'
    ? {
      checking: 'Checking the file…',
      unpacking: 'Unpacking…',
      notAModel: `This file isn't a Lectrice voice model. Pick ${listed(shown.map((bundle) => bundle.archiveName))}.`,
      unreadable: "This file couldn't be opened.",
      pickAnother: 'Pick another file…',
    }
    : {
      checking: 'Checking the folder…',
      unpacking: 'Copying the voice files…',
      notAModel: `This folder isn't a Lectrice voice model. Pick the unpacked ${listed(shown.map((bundle) => bundle.folderName))} folder.`,
      unreadable: "This folder couldn't be opened.",
      pickAnother: 'Pick another folder…',
    };
};

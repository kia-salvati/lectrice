import type { ModelStatus } from '../commands/types';
import { percentOf } from './format.ts';

/** True while no bundle is installed: the reader has nothing to speak with. */
export const hasNoVoice = (status: ModelStatus): boolean => !status.bundles.some((bundle) => bundle.installed);

/**
 * The reader pill's status while there is no voice: "Getting the voice · 41 %" while it downloads, "Getting the
 * voice" before a number is known, otherwise "No voice installed". Null once a voice is installed (the engine's own
 * state applies then).
 */
export const voiceStatusText = (status: ModelStatus): string | null => {
  if (!hasNoVoice(status)) return null;
  const { transfer } = status;
  switch (transfer.kind) {
    case 'downloading':
    case 'retrying':
    case 'waitingForNetwork':
      return `Getting the voice · ${percentOf(transfer.doneBytes, transfer.totalBytes)} %`;
    case 'starting':
    case 'verifying':
      return 'Getting the voice';
    default:
      return 'No voice installed';
  }
};

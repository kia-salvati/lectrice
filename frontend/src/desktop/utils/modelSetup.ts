import type { EngineStatus, ModelBundle, ModelStatus } from '../commands/types';

/** The core keeps this much free beyond the archive and the unpacked files while installing (spec §9.2). */
export const INSTALL_SPARE_BYTES = 200 * 1_000_000;

/** What the first-run column shows when no download or import is running: the choice, or the engine's own state. */
export type InstallPhase = 'choosing' | 'engineLoading' | 'engineFailed' | 'ready';

/** An installed bundle without an engine ("missing") can only be helped by activating it again, like a failed start. */
const ENGINE_PHASE: Record<EngineStatus, InstallPhase> = { missing: 'engineFailed', loading: 'engineLoading', ready: 'ready', failed: 'engineFailed' };

export const installPhase = (status: ModelStatus): InstallPhase => (status.bundles.some((bundle) => bundle.installed) ? ENGINE_PHASE[status.engine] : 'choosing');

/** Free space the install of a bundle needs: the whole archive, the unpacked files and the spare (spec §9.2). */
export const installNeedsBytes = (bundle: ModelBundle): number => bundle.downloadBytes + bundle.installedBytes + INSTALL_SPARE_BYTES;

export const bundleById = (status: ModelStatus, id: string | null): ModelBundle | null => status.bundles.find((bundle) => bundle.id === id) ?? null;

/** The Smaller bundle when it would still install in the free space; what the no-space message offers instead. */
export const smallerThatFits = (status: ModelStatus, failedBundle: ModelBundle | null): ModelBundle | null => {
  if (failedBundle === null || failedBundle.variant !== 'fp32') return null;
  const smaller = status.bundles.find((bundle) => bundle.variant === 'int8') ?? null;
  return smaller !== null && status.freeBytes !== null && installNeedsBytes(smaller) <= status.freeBytes ? smaller : null;
};

/** A transfer blocks a second one, so "Import from file…" waits for it. */
export const isTransferBusy = (status: ModelStatus): boolean => status.transfer.kind !== 'none' && status.transfer.kind !== 'failed';

import { useCallback, useEffect, useState } from 'react';
import type { DesktopCommands, ModelBundleId, ModelImportKind, ModelStatus } from '../commands/types';

export interface ModelSetup {
  /** Null until the shell first answers. */
  status: ModelStatus | null;
  /** The radio card picked on the first-run screen (page state, not the shell's). */
  selectedBundleId: ModelBundleId | null;
  select: (bundleId: ModelBundleId) => void;
  download: (bundleId: ModelBundleId) => void;
  pause: () => void;
  cancel: () => void;
  importModel: (kind: ModelImportKind) => void;
  /** What the last import asked for: the shell's status says `source: 'import'` for a file and a folder alike. */
  importKind: ModelImportKind;
  activate: (bundleId: ModelBundleId) => void;
  deleteBundle: (bundleId: ModelBundleId) => void;
  /** The bundle whose delete was refused because an export uses it, until the message is closed. */
  refusedDelete: ModelBundleId | null;
  dismissRefusedDelete: () => void;
  dismissSetup: () => void;
}

/** Failures of the work itself come back in the status; a refused command is a bug or a race, so it is logged. */
const report = (command: string) => (cause: unknown) => console.error(`Desktop command ${command} failed`, cause);

/** The voice-model state from the shell (one read, then the `model-status` event) and the actions on it. */
export const useModelSetup = (commands: DesktopCommands): ModelSetup => {
  const [status, setStatus] = useState<ModelStatus | null>(null);
  const [selectedBundleId, setSelectedBundleId] = useState<ModelBundleId | null>(null);
  const [refusedDelete, setRefusedDelete] = useState<ModelBundleId | null>(null);
  const [importKind, setImportKind] = useState<ModelImportKind>('archive');

  useEffect(() => {
    let live = true;
    let heardEvent = false;
    const stop = commands.onModelStatus((next) => {
      heardEvent = true;
      setStatus(next);
    });
    // An event that came first is newer than this answer.
    commands.modelStatus().then((first) => {
      if (live && !heardEvent) setStatus(first);
    }).catch(report('model_status'));
    return () => {
      live = false;
      stop();
    };
  }, [commands]);

  const download = useCallback((bundleId: ModelBundleId) => {
    commands.modelDownloadStart(bundleId).catch(report('model_download_start'));
  }, [commands]);
  const pause = useCallback(() => {
    commands.modelDownloadPause().catch(report('model_download_pause'));
  }, [commands]);
  const cancel = useCallback(() => {
    commands.modelDownloadCancel().catch(report('model_download_cancel'));
  }, [commands]);
  // Set before the dialog opens, since the import's status can come before the command answers.
  const importModel = useCallback((kind: ModelImportKind) => {
    const before = importKind;
    setImportKind(kind);
    commands.modelImport(kind).then((picked) => {
      if (!picked) setImportKind(before);
    }).catch(report('model_import'));
  }, [commands, importKind]);
  const activate = useCallback((bundleId: ModelBundleId) => {
    commands.modelActivate(bundleId).catch(report('model_activate'));
  }, [commands]);
  const deleteBundle = useCallback((bundleId: ModelBundleId) => {
    commands.modelDelete(bundleId).then((result) => {
      if (!result.deleted) setRefusedDelete(bundleId);
    }).catch(report('model_delete'));
  }, [commands]);
  const dismissSetup = useCallback(() => {
    commands.modelSetupDismiss().catch(report('model_setup_dismiss'));
  }, [commands]);
  const dismissRefusedDelete = useCallback(() => setRefusedDelete(null), []);

  return { status, selectedBundleId, select: setSelectedBundleId, download, pause, cancel, importModel, importKind, activate, deleteBundle, refusedDelete, dismissRefusedDelete, dismissSetup };
};

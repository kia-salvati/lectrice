import { useCallback, useState } from 'react';
import type { DesktopCommands, ModelBundleId, ModelImportKind, ModelStatus } from '../commands/types';
import type { ReportCommandError } from './useCommandErrors';
import { useShellStatus } from './useShellStatus';
import type { ShellStatusSource } from './useShellStatus';

export interface ModelSetup {
  /** Null until the shell first answers. */
  status: ModelStatus | null;
  /** The first `model_status` failed: the window shows a failed state with `retryLoad`. */
  loadFailed: boolean;
  retryLoad: () => void;
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

const MODEL_STATUS: ShellStatusSource<ModelStatus> = {
  command: 'model_status',
  read: (commands) => commands.modelStatus(),
  subscribe: (commands, listener) => commands.onModelStatus(listener),
};

/** The voice-model state from the shell and the actions on it. Failures of the work itself come back in the status; a refused command goes to `report`. */
export const useModelSetup = (commands: DesktopCommands, report: ReportCommandError): ModelSetup => {
  const { status, loadFailed, retry: retryLoad } = useShellStatus(commands, MODEL_STATUS);
  const [selectedBundleId, setSelectedBundleId] = useState<ModelBundleId | null>(null);
  const [refusedDelete, setRefusedDelete] = useState<ModelBundleId | null>(null);
  const [importKind, setImportKind] = useState<ModelImportKind>('archive');

  const download = useCallback((bundleId: ModelBundleId) => {
    commands.modelDownloadStart(bundleId).catch(report('model_download_start', "Couldn't start the download"));
  }, [commands, report]);
  const pause = useCallback(() => {
    commands.modelDownloadPause().catch(report('model_download_pause', "Couldn't pause the download"));
  }, [commands, report]);
  const cancel = useCallback(() => {
    commands.modelDownloadCancel().catch(report('model_download_cancel', "Couldn't cancel the download"));
  }, [commands, report]);
  // Set before the dialog opens, since the import's status can come before the command answers.
  const importModel = useCallback((kind: ModelImportKind) => {
    const before = importKind;
    setImportKind(kind);
    commands.modelImport(kind).then((picked) => {
      if (!picked) setImportKind(before);
    }).catch((cause: unknown) => {
      setImportKind(before);
      report('model_import', "Couldn't open the import")(cause);
    });
  }, [commands, importKind, report]);
  const activate = useCallback((bundleId: ModelBundleId) => {
    commands.modelActivate(bundleId).catch(report('model_activate', "Couldn't switch the voice model"));
  }, [commands, report]);
  const deleteBundle = useCallback((bundleId: ModelBundleId) => {
    commands.modelDelete(bundleId).then((result) => {
      if (!result.deleted) setRefusedDelete(bundleId);
    }).catch(report('model_delete', "Couldn't delete the voice model"));
  }, [commands, report]);
  const dismissSetup = useCallback(() => {
    commands.modelSetupDismiss().catch(report('model_setup_dismiss', "Couldn't save your choice"));
  }, [commands, report]);
  const dismissRefusedDelete = useCallback(() => setRefusedDelete(null), []);

  return { status, loadFailed, retryLoad, selectedBundleId, select: setSelectedBundleId, download, pause, cancel, importModel, importKind, activate, deleteBundle, refusedDelete, dismissRefusedDelete, dismissSetup };
};

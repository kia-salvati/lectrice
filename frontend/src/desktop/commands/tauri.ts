import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AudioExport } from '../../types/api';
import type { AppInfo, DesktopCommands, DesktopOs, Licenses, ModelDeleteResult, ModelStatus, SaveAsResult } from './types';
import { toDesktopError, toExportAvailability, toModelStatus } from './wire';
import type { ExportAvailabilityDto, ModelStatusDto } from './wire';

const MODEL_STATUS_EVENT = 'model-status';

interface AppInfoDto { version: string; coreVersion: string; os: DesktopOs; arch: string; }

/** `invoke`, with every rejection turned into a `DesktopError`. */
const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toDesktopError(cause);
  }
};

/**
 * The desktop shell's commands over Tauri's IPC (desktop spec §4.4). The model status reaches the listeners only from
 * the `model-status` event, which the shell sends on every change in order; the statuses the commands answer with
 * could arrive out of order with it, so they are dropped. One event listener, registered when the page loads.
 */
export const createTauriDesktopCommands = (): DesktopCommands => {
  const listeners = new Set<(status: ModelStatus) => void>();
  listen<ModelStatusDto>(MODEL_STATUS_EVENT, (event) => {
    const status = toModelStatus(event.payload);
    listeners.forEach((listener) => listener(status));
  }).catch((cause: unknown) => console.error('Cannot listen for the model status', cause));

  const statusCommand = async (command: string, args?: Record<string, unknown>): Promise<void> => {
    await call<unknown>(command, args);
  };

  let info: Promise<AppInfo> | null = null;
  const appInfo = (): Promise<AppInfo> => {
    info ??= call<AppInfoDto>('app_info').then(({ version, ...rest }) => ({ appVersion: version, ...rest }));
    return info;
  };

  return {
    appInfo,
    licenses: () => call<Licenses>('licenses'),

    modelStatus: async () => toModelStatus(await call<ModelStatusDto>('model_status')),

    onModelStatus(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    modelDownloadStart: (bundleId) => statusCommand('model_download_start', { bundleId }),
    modelDownloadPause: () => statusCommand('model_download_pause'),
    modelDownloadCancel: () => statusCommand('model_download_cancel'),
    modelImport: async (kind) => (await call<ModelStatusDto | null>('model_import', { kind })) !== null,
    modelActivate: (bundleId) => statusCommand('model_activate', { bundleId }),
    async modelDelete(bundleId) {
      const { deleted, blockingExports } = await call<ModelDeleteResult>('model_delete', { bundleId });
      return { deleted, blockingExports };
    },
    modelSetupDismiss: () => statusCommand('model_setup_dismiss'),

    exportList: () => call<AudioExport[]>('export_list'),
    exportSaveAs: (exportId) => call<SaveAsResult>('export_save_as', { exportId }),
    exportReveal: (exportId) => call<void>('export_reveal', { exportId }),
    async exportAvailability() {
      const [dto, { os }] = await Promise.all([call<ExportAvailabilityDto>('export_availability'), appInfo()]);
      return toExportAvailability(dto, os);
    },

    openLink: (linkId) => call<void>('open_link', { linkId }),
  };
};

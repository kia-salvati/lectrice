import { convertFileSrc } from '@tauri-apps/api/core';
import { createTauriDesktopCommands } from '../desktop/commands/tauri';
import type { Platform } from './types';

const API_SCHEME = 'lectrice';
const API_PATH = 'api';

/** The desktop shell serves the API on its own in-process scheme, whose URL differs per OS. Export is M4B only. */
export const desktopPlatform: Platform = {
  apiBaseUrl: convertFileSrc(API_PATH, API_SCHEME),
  exportFormats: ['m4b'],
  desktop: { commands: createTauriDesktopCommands(), loadUi: () => import('../desktop/ui') },
};

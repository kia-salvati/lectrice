import type { Platform } from './types';
import { webPlatform } from './web';

// Vite replaces VITE_PLATFORM at build time, so the web build drops the desktop branch and never ships @tauri-apps.
const loadPlatform = async (): Promise<Platform> => {
  const kind = import.meta.env.VITE_PLATFORM;
  if (kind === 'desktop') return (await import('./desktop')).desktopPlatform;
  if (kind === undefined || kind === 'web') return webPlatform;
  throw new Error(`VITE_PLATFORM must be "web" or "desktop", got "${kind}"`);
};

export const platform: Platform = await loadPlatform();

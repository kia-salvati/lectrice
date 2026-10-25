import type { Platform } from './types';

/** Same origin: the Vite dev server proxies /api to the backend. */
export const webPlatform: Platform = { apiBaseUrl: '/api', exportFormats: ['mp3', 'm4b'], desktop: null };

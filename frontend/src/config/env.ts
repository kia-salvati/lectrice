import { platform } from '../platform';

export type ApiMode = 'mock' | 'http';

const API_MODES: readonly ApiMode[] = ['mock', 'http'];

const parseApiMode = (value: string | undefined): ApiMode => {
  const mode = (value ?? '').trim().toLowerCase();
  const match = API_MODES.find((candidate) => candidate === mode);
  if (!match) throw new Error(`VITE_API_MODE must be "mock" or "http", got "${value ?? ''}"`);
  return match;
};

export const API_MODE: ApiMode = parseApiMode(import.meta.env.VITE_API_MODE);

export const API_BASE_PATH = platform.apiBaseUrl;

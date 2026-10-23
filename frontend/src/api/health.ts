import type { Health } from '../types/api';
import { requestJson } from './client';

export const getHealth = (signal?: AbortSignal): Promise<Health> => requestJson<Health>('/health', { signal });

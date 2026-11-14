import type { ErrorBody } from '../types/errors';

/** The inner `{ name, code, message }` of an `{ error: … }` answer, or null if it has another shape. */
export const readErrorBody = (value: unknown): ErrorBody | null => {
  if (typeof value !== 'object' || value === null || !('error' in value)) return null;
  const inner: unknown = value.error;
  if (typeof inner !== 'object' || inner === null || !('name' in inner) || !('code' in inner) || !('message' in inner)) return null;
  const { name, code, message } = inner;
  return typeof name === 'string' && typeof code === 'number' && typeof message === 'string' ? { name, code, message } : null;
};

import { API_BASE_PATH } from '../config/env';
import type { ApiErrorBody } from '../types/api';
import { ApiError, ClientError, isAbortError, isApiErrorName } from './errors';

const isApiErrorBody = (value: unknown): value is ApiErrorBody => {
  if (typeof value !== 'object' || value === null || !('error' in value)) return false;
  const inner: unknown = value.error;
  return typeof inner === 'object' && inner !== null && 'name' in inner && 'code' in inner && 'message' in inner
    && typeof inner.name === 'string' && typeof inner.code === 'number' && typeof inner.message === 'string';
};

const toApiError = async (response: Response): Promise<ApiError> => {
  const body: unknown = await response.json().catch(() => null);
  if (isApiErrorBody(body) && isApiErrorName(body.error.name)) {
    return new ApiError(body.error.name, body.error.code, body.error.message, response.status);
  }
  return new ApiError('Internal', 9999, 'Something went wrong.', response.status);
};

/** The only place that calls fetch. Throws ApiError (non-2xx), ClientError('NetworkError'), or re-throws AbortError untouched. */
const send = async (path: string, init: RequestInit = {}): Promise<Response> => {
  let response: Response;
  try {
    response = await fetch(`${API_BASE_PATH}${path}`, init);
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw new ClientError('NetworkError', { cause: error });
  }
  if (!response.ok) throw await toApiError(response);
  return response;
};

export const requestJson = async <T>(path: string, init?: RequestInit): Promise<T> => {
  const response = await send(path, init);
  try {
    return (await response.json()) as T;
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw new ClientError('UnexpectedResponse', { cause: error });
  }
};

export const requestBlob = async (path: string, init?: RequestInit): Promise<Blob> => (await send(path, init)).blob();

export const requestVoid = async (path: string, init?: RequestInit): Promise<void> => {
  await send(path, init);
};

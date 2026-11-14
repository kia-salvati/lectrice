import { API_BASE_PATH } from '../config/env';
import { toApiError } from './errorResponse';
import { ClientError, isAbortError } from './errors';

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

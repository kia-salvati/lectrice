// Extensions spelled out: `node --test` runs this file directly, without a bundler.
import { INTERNAL_ERROR, isRegistryErrorName } from '../config/errorRegistry.ts';
import { readErrorBody } from '../utils/errorBody.ts';
import { ApiError } from './errors.ts';

/** A non-2xx answer as an `ApiError`. A name this build doesn't know (a newer server) keeps the server's message and code. */
export const toApiError = async (response: Response): Promise<ApiError> => {
  const body = readErrorBody(await response.json().catch(() => null));
  if (!body) return new ApiError(INTERNAL_ERROR.name, INTERNAL_ERROR.code, INTERNAL_ERROR.message, response.status);
  const name = isRegistryErrorName(body.name) ? body.name : INTERNAL_ERROR.name;
  return new ApiError(name, body.code, body.message, response.status);
};

import { ERROR_CODES } from '../../config/errorRegistry';
import type { ApiErrorName } from '../../types/errors';
import { ApiError } from '../errors';

/** An answer the backend would give: the registry's code with the message and HTTP status it sends. */
export const mockError = (name: ApiErrorName, message: string, status: number): ApiError => new ApiError(name, ERROR_CODES[name], message, status);

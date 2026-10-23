import type { ApiErrorName, AppErrorName, ClientErrorName } from '../types/errors';

const API_ERROR_NAMES: readonly ApiErrorName[] = ['RequestInvalid', 'UploadMissingFile', 'UploadTooLarge', 'PdfInvalid', 'PdfEncrypted', 'PdfNoText', 'RouteNotFound', 'MethodNotAllowed', 'RequestCancelled', 'DocumentNotFound', 'SentenceNotFound', 'VoiceUnknown', 'SpeechEngineUnavailable', 'SpeechSynthesisFailed', 'ExportNotFound', 'ExportNotReady', 'ExportFormatUnsupported', 'ExporterUnavailable', 'StorageFailed', 'Internal'];

const CLIENT_ERROR_MESSAGES: Record<ClientErrorName, string> = {
  NetworkError: "Can't reach the Lectrice server. Is it running?",
  UnexpectedResponse: 'The server sent an answer Lectrice did not understand.',
  FileNotPdf: 'This file is not a PDF.',
  FileEmpty: 'This file is empty.',
  FileTooLarge: 'This file is too large.',
  AudioPlaybackFailed: "This sentence's audio could not be played.",
};

/** A non-2xx answer from the backend, carrying the registry name, code and user-facing message. */
export class ApiError extends Error {
  override readonly name: ApiErrorName;
  readonly code: number;
  readonly status: number;

  constructor(name: ApiErrorName, code: number, message: string, status: number) {
    super(message);
    this.name = name;
    this.code = code;
    this.status = status;
  }
}

/** A failure detected in the browser (no server answer, bad file, playback failure). */
export class ClientError extends Error {
  override readonly name: ClientErrorName;

  constructor(name: ClientErrorName, options?: { cause?: unknown }) {
    super(CLIENT_ERROR_MESSAGES[name], options);
    this.name = name;
  }
}

export type AppError = ApiError | ClientError;

export const isApiErrorName = (value: string): value is ApiErrorName => (API_ERROR_NAMES as readonly string[]).includes(value);

export const isAbortError = (error: unknown): boolean => error instanceof DOMException && error.name === 'AbortError';

export const hasErrorName = (error: unknown, name: AppErrorName): boolean => (error instanceof ApiError || error instanceof ClientError) && error.name === name;

/** Anything thrown that is not already an AppError becomes UnexpectedResponse, keeping the original as `cause`. */
export const toAppError = (error: unknown): AppError => (error instanceof ApiError || error instanceof ClientError) ? error : new ClientError('UnexpectedResponse', { cause: error });

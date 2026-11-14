/** The core's error registry (`lectrice-domain/src/error.rs`): name → code. `tests/errorRegistry.test.ts` checks it against the Rust file. */
export const ERROR_CODES = {
  RequestInvalid: 1000,
  UploadMissingFile: 1001,
  UploadTooLarge: 1002,
  PdfInvalid: 1003,
  PdfEncrypted: 1004,
  PdfNoText: 1005,
  RouteNotFound: 1006,
  MethodNotAllowed: 1007,
  RequestCancelled: 1008,
  RequestForbidden: 1009,
  PdfTooComplex: 1010,
  DocumentNotFound: 1101,
  SentenceNotFound: 1102,
  VoiceUnknown: 2001,
  SpeechEngineUnavailable: 2002,
  SpeechSynthesisFailed: 2003,
  ExportNotFound: 3001,
  ExportNotReady: 3002,
  ExportFormatUnsupported: 3003,
  ExporterUnavailable: 3004,
  AudiobookNotLectrice: 3101,
  AudiobookOtherBook: 3102,
  AudiobookUnreadable: 3103,
  ModelMissing: 4001,
  ModelDownloadFailed: 4002,
  ModelChecksumMismatch: 4003,
  StorageFull: 4004,
  ExportPaused: 4005,
  FileUnreadable: 4006,
  HelperNotPaired: 5001,
  HelperPairingFailed: 5002,
  HelperUnreachable: 5003,
  HelperModelMissing: 5004,
  HelperIncompatible: 5005,
  HelperBusy: 5006,
  StorageFailed: 9001,
  Internal: 9999,
} as const;

export type RegistryErrorName = keyof typeof ERROR_CODES;

export const isRegistryErrorName = (value: string): value is RegistryErrorName => Object.hasOwn(ERROR_CODES, value);

export const CLIENT_ERROR_NAMES = ['NetworkError', 'UnexpectedResponse', 'FileNotPdf', 'FileEmpty', 'FileTooLarge', 'AudioPlaybackFailed'] as const;

/** The registry's fallback, used when an answer carries no readable error body. */
export const INTERNAL_ERROR = { name: 'Internal', code: ERROR_CODES.Internal, message: 'Something went wrong.' } as const;

/** Server error names (spec §8). */
export type ApiErrorName =
  | 'RequestInvalid' | 'UploadMissingFile' | 'UploadTooLarge' | 'PdfInvalid' | 'PdfEncrypted' | 'PdfNoText' | 'RouteNotFound' | 'MethodNotAllowed' | 'RequestCancelled'
  | 'DocumentNotFound' | 'SentenceNotFound'
  | 'VoiceUnknown' | 'SpeechEngineUnavailable' | 'SpeechSynthesisFailed'
  | 'ExportNotFound' | 'ExportNotReady' | 'ExportFormatUnsupported' | 'ExporterUnavailable'
  | 'StorageFailed' | 'Internal';

/** Errors raised in the browser, before or instead of a server answer. */
export type ClientErrorName = 'NetworkError' | 'UnexpectedResponse' | 'FileNotPdf' | 'FileEmpty' | 'FileTooLarge' | 'AudioPlaybackFailed';

export type AppErrorName = ApiErrorName | ClientErrorName;

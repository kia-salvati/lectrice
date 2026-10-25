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

/** Names a desktop command can reject with: the same registry as the server's, plus the names of its model and storage errors (desktop spec §4.4, §9.2). */
export type DesktopErrorName = ApiErrorName | 'ModelMissing' | 'ModelDownloadFailed' | 'ModelChecksumMismatch' | 'StorageFull' | 'ExportPaused' | 'FileUnreadable';

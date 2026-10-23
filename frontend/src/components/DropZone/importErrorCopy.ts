import type { AppError } from '../../api/errors';
import { MAX_UPLOAD_MB } from '../../config/constants';
import type { AppErrorName } from '../../types/errors';

interface ImportErrorCopy { title: string; body: string; }

const TOO_LARGE: ImportErrorCopy = { title: 'This file is too large', body: `is bigger than ${MAX_UPLOAD_MB} MB, the largest file Lectrice accepts.` };

/** Title and body per error. The body follows the bold file name. */
const COPY: Partial<Record<AppErrorName, ImportErrorCopy>> = {
  PdfNoText: { title: 'This PDF has no selectable text', body: 'looks like a scanned document. There is no text layer for Lectrice to read, so it cannot be read aloud.' },
  PdfEncrypted: { title: 'This PDF is password-protected', body: 'is locked. Remove the password in a PDF viewer, then open it again.' },
  PdfInvalid: { title: "This file isn't a readable PDF", body: 'could not be read. It may be damaged, or not really a PDF.' },
  FileNotPdf: { title: 'This file is not a PDF', body: 'is not a PDF. Lectrice can only read PDF files.' },
  FileEmpty: { title: 'This file is empty', body: 'has no content.' },
  FileTooLarge: TOO_LARGE,
  UploadTooLarge: TOO_LARGE,
  NetworkError: { title: "Can't reach Lectrice", body: 'could not be sent. Check that the Lectrice server is running, then try again.' },
};

export const importErrorCopy = (error: AppError): ImportErrorCopy => COPY[error.name] ?? { title: "Couldn't open this PDF", body: error.message };

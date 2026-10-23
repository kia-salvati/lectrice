import type { ClientErrorName } from '../types/errors';

export interface FileFacts { name: string; type: string; size: number; }

const PDF_MIME = 'application/pdf';
const PDF_EXTENSION = /\.pdf$/i;

/** First checks that need no reading. Some systems report an empty MIME type, so the extension also counts. */
export const checkPdfFile = (file: FileFacts, maxBytes: number): ClientErrorName | null => {
  const looksLikePdf = file.type === PDF_MIME || (file.type === '' && PDF_EXTENSION.test(file.name.trim()));
  if (!looksLikePdf) return 'FileNotPdf';
  if (file.size === 0) return 'FileEmpty';
  if (file.size > maxBytes) return 'FileTooLarge';
  return null;
};

/** Every PDF starts with "%PDF-". */
export const PDF_MAGIC = '%PDF-';

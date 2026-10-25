import type { DesktopOs, LicenseTextId } from '../../commands/types';

export interface LicenseRow {
  component: string;
  license: string;
  /** Its text comes from the app's licenses folder (the `licenses` command). */
  textId: LicenseTextId;
  /** FFmpeg is bundled only on Windows; Linux uses the system's. */
  onlyOn?: DesktopOs;
}

export const LICENSES: readonly LicenseRow[] = [
  { component: 'Lectrice', license: 'MIT OR Apache-2.0', textId: 'lectrice' },
  { component: 'Kokoro 82M model and voices', license: 'Apache-2.0', textId: 'kokoro' },
  { component: 'sherpa-onnx', license: 'Apache-2.0', textId: 'sherpaOnnx' },
  { component: 'espeak-ng and espeak-ng-data', license: 'GPL-3.0', textId: 'espeakNg' },
  { component: 'onnxruntime', license: 'MIT', textId: 'onnxruntime' },
  { component: 'PDFium', license: 'BSD-3-Clause and others', textId: 'pdfium' },
  { component: 'FFmpeg', license: 'LGPL-2.1-or-later', textId: 'ffmpeg', onlyOn: 'windows' },
  { component: 'Tauri and its plugins', license: 'MIT OR Apache-2.0', textId: 'tauri' },
  { component: 'React', license: 'MIT', textId: 'react' },
];

export const licensesFor = (os: DesktopOs): LicenseRow[] => LICENSES.filter((row) => row.onlyOn === undefined || row.onlyOn === os);

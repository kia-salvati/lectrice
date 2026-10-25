import type { AppInfo, DesktopOs } from '../commands/types';

const OS_LABEL: Record<DesktopOs, string> = { linux: 'Linux', windows: 'Windows' };

/** "Version 0.1.0 · core and UI 0.3.0 · Linux x64": the About tab's line, also what "Copy details" copies for bug reports. */
export const versionLine = (info: AppInfo): string => `Version ${info.appVersion} · core and UI ${info.coreVersion} · ${OS_LABEL[info.os]} ${info.arch}`;

const MB = 1_000_000;
const GB = 1_000 * MB;
const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;

/** "350 MB" */
export const formatMb = (bytes: number): string => `${Math.round(bytes / MB)} MB`;

/** "23.4 GB" above a gigabyte, else "610 MB": the free-space line. */
export const formatSpace = (bytes: number): string => (bytes >= GB ? `${(bytes / GB).toFixed(1)} GB` : formatMb(bytes));

/** "142 MB of 350 MB" */
export const formatProgressMb = (doneBytes: number, totalBytes: number): string => `${Math.floor(doneBytes / MB)} MB of ${formatMb(totalBytes)}`;

/** "2.1 MB/s" */
export const formatSpeed = (bytesPerSecond: number): string => `${(bytesPerSecond / MB).toFixed(1)} MB/s`;

export const percentOf = (done: number, total: number): number => (total <= 0 ? 0 : Math.min(100, Math.round((done / total) * 100)));

/** "1 min", "45 min", "8 h", "8 h 10 min": minutes rounded up. */
export const formatDuration = (seconds: number): string => {
  const minutes = Math.max(1, Math.ceil(seconds / SECONDS_PER_MINUTE));
  if (minutes < MINUTES_PER_HOUR) return `${minutes} min`;
  const hours = Math.floor(minutes / MINUTES_PER_HOUR);
  const rest = minutes % MINUTES_PER_HOUR;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
};

/** "under a minute left", "about 2 min left", "about 1 h 5 min left", "about 2 h left" */
export const formatApproxTimeLeft = (seconds: number): string => (seconds < SECONDS_PER_MINUTE ? 'under a minute left' : `about ${formatDuration(seconds)} left`);

/** The core writes M4B audio at 64 kbit/s (lectrice-infra `export/ffmpeg.rs`), so the size gives the length closely. */
const M4B_BYTES_PER_SECOND = 64_000 / 8;

/** The length of a finished audiobook file, from its size. */
export const m4bSeconds = (fileBytes: number): number => fileBytes / M4B_BYTES_PER_SECOND;

/** "30 s", "2 min": the offline retry countdown. */
export const formatRetryIn = (seconds: number): string => (seconds >= SECONDS_PER_MINUTE ? `${Math.ceil(seconds / SECONDS_PER_MINUTE)} min` : `${Math.ceil(seconds)} s`);

export const formatCount = (value: number): string => value.toLocaleString('en');

/** The last folder of a path, on either separator: "Music" for "~/Music/Book.m4b" and "C:\Users\Sam\Music\Book.m4b". */
export const folderNameOf = (path: string): string => {
  const parts = path.split(/[\\/]/).filter((part) => part !== '');
  return parts.length >= 2 ? parts[parts.length - 2] : '';
};

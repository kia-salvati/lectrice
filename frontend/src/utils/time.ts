const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;

/** "under a minute left", "12 min left", "3 h 05 min left" */
export const formatTimeLeft = (seconds: number): string => {
  const minutes = Math.ceil(seconds / SECONDS_PER_MINUTE);
  if (minutes <= 1) return 'under a minute left';
  if (minutes < MINUTES_PER_HOUR) return `${minutes} min left`;
  const hours = Math.floor(minutes / MINUTES_PER_HOUR);
  const rest = String(minutes % MINUTES_PER_HOUR).padStart(2, '0');
  return `${hours} h ${rest} min left`;
};

const UNITS: ReadonlyArray<[Intl.RelativeTimeFormatUnit, number]> = [['year', 31_536_000], ['month', 2_592_000], ['week', 604_800], ['day', 86_400], ['hour', 3_600], ['minute', 60]];

const relative = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

/** "just now", "2 hours ago", "yesterday", "last week". `now` is passed in so the function stays pure. */
export const formatRelativeTime = (isoDate: string, now: Date): string => {
  const elapsedSeconds = (now.getTime() - new Date(isoDate).getTime()) / 1000;
  for (const [unit, size] of UNITS) {
    if (elapsedSeconds >= size) return relative.format(-Math.floor(elapsedSeconds / size), unit);
  }
  return 'just now';
};

// Extensions spelled out: `node --test` runs this file directly, without a bundler.
import type { PairedPhone, PhoneHelperStatus } from '../commands/types.ts';

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const DAYS_SHOWN_AS_COUNT = 7;

const THIN_SPACE = ' ';
const CODE_GROUP_LENGTH = 3;
/** The third wrong code closes the pairing window. */
const WRONG_CODES_ALLOWED = 3;

export const PORT_MIN = 1024;
export const PORT_MAX = 65535;

/** "482 913", with a thin space between the groups. */
export const groupCode = (code: string): string => `${code.slice(0, CODE_GROUP_LENGTH)}${THIN_SPACE}${code.slice(CODE_GROUP_LENGTH)}`;

/** "Pairing code 4 8 2 9 1 3": a screen reader says the digits one by one. */
export const codeAriaLabel = (code: string): string => `Pairing code ${code.split('').join(' ')}`;

/** "1:05", "0:00" once the time is up. */
export const formatCountdown = (msLeft: number): string => {
  const seconds = Math.max(0, Math.ceil(msLeft / SECOND));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
};

/** How full the countdown bar is, 100 to 0. */
export const countdownPercent = (msLeft: number, totalSeconds: number): number => (totalSeconds <= 0 ? 0 : Math.min(100, Math.max(0, (msLeft / (totalSeconds * SECOND)) * 100)));

/** "A wrong code was typed (1 of 3)." */
export const wrongCodesText = (wrongCodes: number): string => `A wrong code was typed (${wrongCodes} of ${WRONG_CODES_ALLOWED}).`;

const listed = (items: readonly string[]): string => (items.length === 1 ? items[0] : `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`);

/** "A device at 192.168.1.34 is pairing…": the addresses with a pairing attempt under way. */
export const tryingText = (addresses: readonly string[]): string => (addresses.length === 1 ? `A device at ${addresses[0]} is pairing…` : `Devices at ${listed(addresses)} are pairing…`);

/** Whole minutes until pairing is allowed again, rounded up. */
export const coolDownMinutes = (until: number, now: number): number => Math.max(1, Math.ceil((until - now) / MINUTE));

/** "Last seen 5 min ago", "Last seen yesterday", "Last seen 12 Nov"; "Paired, not connected yet" before the first session. */
export const lastSeenText = (lastSeenAt: string | null, now: number): string => {
  if (lastSeenAt === null) return 'Paired, not connected yet';
  const ago = now - Date.parse(lastSeenAt);
  if (ago < MINUTE) return 'Last seen just now';
  if (ago < HOUR) return `Last seen ${Math.floor(ago / MINUTE)} min ago`;
  if (ago < DAY) return `Last seen ${Math.floor(ago / HOUR)} h ago`;
  const days = Math.floor(ago / DAY);
  if (days === 1) return 'Last seen yesterday';
  if (days < DAYS_SHOWN_AS_COUNT) return `Last seen ${days} days ago`;
  return `Last seen ${new Date(lastSeenAt).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}`;
};

/** The line under a phone's name; while the helper is off every phone reads "Last seen …". */
export const phoneMetaText = (phone: PairedPhone, helperOn: boolean, now: number): string => {
  if (helperOn && phone.state === 'making') return 'Connected · making audio';
  if (helperOn && phone.state === 'connected') return 'Connected';
  return lastSeenText(phone.lastSeenAt, now);
};

/** The phones this computer is making audio for right now. */
export const makingPhones = (status: PhoneHelperStatus | null): readonly PairedPhone[] => status?.phones.filter((phone) => phone.state === 'making') ?? [];

/** The chip: "Galaxy S10+", "Galaxy S10+ and 1 more". */
export const chipText = (making: readonly PairedPhone[]): string => (making.length === 1 ? making[0].name : `${making[0].name} and ${making.length - 1} more`);

/** The chip's title and the reader's line: "Making audio for Galaxy S10+", "Making audio for 2 phones". */
export const makingAudioText = (making: readonly PairedPhone[]): string => (making.length === 1 ? `Making audio for ${making[0].name}` : `Making audio for ${making.length} phones`);

/** The network's name goes after "network" when it is known. */
export const otherNetworkText = (name: string | null): string => `Not on your home network${name === null ? '' : ` (${name})`}, so your phone can't use this computer here.`;

/** The typed port as a number, or null when it isn't a whole number from 1024 to 65535. */
export const parsePort = (text: string): number | null => {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const port = Number(trimmed);
  return port >= PORT_MIN && port <= PORT_MAX ? port : null;
};

type VersionMismatch = NonNullable<PhoneHelperStatus['versionMismatch']>;

export const versionToastText = ({ update, phoneName }: VersionMismatch): { title: string; body: string } => (update === 'computer'
  ? { title: 'Update Lectrice on this computer', body: `${phoneName} has a newer Lectrice. Update this computer to help it.` }
  : { title: 'Update Lectrice on your phone', body: `${phoneName} has an older Lectrice. Update it to use this computer.` });

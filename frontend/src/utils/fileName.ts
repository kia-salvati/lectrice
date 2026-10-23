// Characters no common file system accepts in a name, plus control characters (\p{Cc}).
const UNSAFE_CHARACTERS = /[\\/:*?"<>|]|\p{Cc}/gu;
const MAX_LENGTH = 120;

/** A download file name that is safe on every platform: unsafe characters become "-", spaces collapse. */
export const safeFileName = (name: string): string => name.replace(UNSAFE_CHARACTERS, '-').replace(/\s+/g, ' ').trim().slice(0, MAX_LENGTH);

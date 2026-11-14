import { useCallback, useEffect, useState } from 'react';
import type { DesktopCommands } from '../commands/types';

/** A status the shell sends as an event on every change and also answers on request. Declared once, at module level. */
export interface ShellStatusSource<T> {
  /** The command's name, for the log. */
  command: string;
  read: (commands: DesktopCommands) => Promise<T>;
  subscribe: (commands: DesktopCommands, listener: (status: T) => void) => () => void;
}

export interface ShellStatus<T> {
  /** Null until the shell first answers. */
  status: T | null;
  /** The first read failed and no event came: the screen offers `retry`. */
  loadFailed: boolean;
  retry: () => void;
}

/** One read, then the event: an event that came first is newer than the read's answer. */
export const useShellStatus = <T>(commands: DesktopCommands, source: ShellStatusSource<T>): ShellStatus<T> => {
  const [status, setStatus] = useState<T | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    let heardEvent = false;
    const stop = source.subscribe(commands, (next) => {
      heardEvent = true;
      setLoadFailed(false);
      setStatus(next);
    });
    source.read(commands).then((first) => {
      if (live && !heardEvent) setStatus(first);
    }).catch((cause: unknown) => {
      console.error(`Desktop command ${source.command} failed`, cause);
      if (live && !heardEvent) setLoadFailed(true);
    });
    return () => {
      live = false;
      stop();
    };
  }, [commands, source, attempt]);

  const retry = useCallback(() => {
    setLoadFailed(false);
    setAttempt((count) => count + 1);
  }, []);

  return { status, loadFailed, retry };
};

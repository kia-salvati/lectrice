import { useCallback, useMemo, useRef, useState } from 'react';
import { INTERNAL_ERROR } from '../../config/errorRegistry';
import { DesktopError } from '../commands/errors';

/** One refused command, as the error toast shows it: what didn't happen, and the shell's reason. */
export interface CommandFailure { id: number; title: string; message: string; }

/** Turns a refused command into a toast; `title` says what didn't happen ("Couldn't rename this computer"). */
export type ReportCommandError = (command: string, title: string) => (cause: unknown) => void;

export interface CommandErrors {
  /** The oldest failure not dismissed yet. */
  current: CommandFailure | null;
  report: ReportCommandError;
  dismiss: (id: number) => void;
}

/** The desktop's one channel for refused commands: each becomes an error toast, oldest first. */
export const useCommandErrors = (): CommandErrors => {
  const [failures, setFailures] = useState<readonly CommandFailure[]>([]);
  const nextId = useRef(0);

  const report = useCallback<ReportCommandError>((command, title) => (cause) => {
    console.error(`Desktop command ${command} failed`, cause);
    nextId.current += 1;
    const failure = { id: nextId.current, title, message: cause instanceof DesktopError ? cause.message : INTERNAL_ERROR.message };
    setFailures((current) => [...current, failure]);
  }, []);

  const dismiss = useCallback((id: number) => setFailures((current) => current.filter((failure) => failure.id !== id)), []);

  return useMemo(() => ({ current: failures[0] ?? null, report, dismiss }), [failures, report, dismiss]);
};

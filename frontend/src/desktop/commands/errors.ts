import type { DesktopErrorName } from '../../types/errors';

/** A failed desktop command, in the registry shape the Tauri side returns (`{ name, code, message }`). */
export class DesktopError extends Error {
  override readonly name: DesktopErrorName;
  readonly code: number;

  constructor(name: DesktopErrorName, code: number, message: string) {
    super(message);
    this.name = name;
    this.code = code;
  }
}

export const hasDesktopErrorName = (error: unknown, name: DesktopErrorName): boolean => error instanceof DesktopError && error.name === name;

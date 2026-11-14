import type { CLIENT_ERROR_NAMES, RegistryErrorName } from '../config/errorRegistry';

/** Names the HTTP server can answer with: the whole core registry. */
export type ApiErrorName = RegistryErrorName;

/** Errors raised in the browser, before or instead of a server answer. */
export type ClientErrorName = (typeof CLIENT_ERROR_NAMES)[number];

export type AppErrorName = ApiErrorName | ClientErrorName;

/** Names a desktop command can reject with: the same registry as the server's. */
export type DesktopErrorName = RegistryErrorName;

/** The `{ name, code, message }` shape both the server and the desktop commands send. */
export interface ErrorBody { name: string; code: number; message: string; }

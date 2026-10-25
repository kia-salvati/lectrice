import type { DesktopCommands } from '../desktop/commands/types';
import type { ExportFormat } from '../types/api';

/** The desktop app's own commands and screens. Loaded only by the desktop build; `import type` keeps it out of the web one. */
export interface DesktopShell {
  commands: DesktopCommands;
  /** The desktop-only components, fetched when first needed. */
  loadUi: () => Promise<typeof import('../desktop/ui')>;
}

/** What differs between the web app and the desktop app. */
export interface Platform {
  /** Base URL of the HTTP API, without a trailing slash. */
  apiBaseUrl: string;
  /** Formats offered for whole-book export, in menu order. The first is the default. */
  exportFormats: readonly ExportFormat[];
  /** Null on the web, which has no shell of its own. */
  desktop: DesktopShell | null;
}

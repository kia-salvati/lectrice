import type { AudioExportState } from '../../hooks/useAudioExport';

/** What a shell's export panel gets from the reader. */
export interface ExportPanelContext {
  exportState: AudioExportState;
  /** The next export's voice and whether it reads code and tables. */
  voiceName: string;
  readSkipped: boolean;
  /** The finished (or latest) job's voice. */
  fileVoiceName: string;
  /** The speech engine is ready. */
  canStart: boolean;
  /** Start, continue or export again, with the reader's voice and settings. */
  onStart: () => void;
  /** About how long the whole book is at 1×, from its length in characters. */
  bookSeconds: number;
}

/** What a shell (the desktop app) puts into the reader in place of the web app's parts. */
export interface ReaderShell {
  renderExportPanel: (context: ExportPanelContext) => React.ReactNode;
  /** In the popover instead of the Voice menu, while no voice is installed. */
  noVoiceNotice: React.ReactNode | null;
  /** After "Open another PDF…" in the popover. */
  extraActions: React.ReactNode;
  /** The pill's status instead of "Speech engine unavailable" while there is no voice; the engine banner stays hidden then. */
  voiceStatus: string | null;
  /** Whether the chapters rail is on screen; false again when the reader closes. */
  onRailShownChange: (shown: boolean) => void;
  /** Open the popover with the export section in view (the export toast's "Open book"), then call `onOpenedAtExport`. */
  openAtExport: boolean;
  onOpenedAtExport: () => void;
}

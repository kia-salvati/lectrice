import { useCallback, useEffect, useState } from 'react';
import type { DesktopCommands, ExportAvailability } from '../commands/types';

export interface ExportAvailabilityState {
  /** Available until the shell says otherwise, so the ffmpeg help never flashes up. */
  availability: ExportAvailability;
  isChecking: boolean;
  checkAgain: () => void;
}

const ASSUMED: ExportAvailability = { available: true };

/** Whether exports can run (ffmpeg found and able to make M4B), and "Check again" after installing it. */
export const useExportAvailability = (commands: DesktopCommands): ExportAvailabilityState => {
  const [availability, setAvailability] = useState<ExportAvailability>(ASSUMED);
  const [isChecking, setIsChecking] = useState(false);

  const check = useCallback(() => commands.exportAvailability().then(setAvailability).catch((cause: unknown) => console.error('Export check failed', cause)), [commands]);

  useEffect(() => {
    check();
  }, [check]);

  const checkAgain = useCallback(() => {
    setIsChecking(true);
    check().finally(() => setIsChecking(false));
  }, [check]);

  return { availability, isChecking, checkAgain };
};

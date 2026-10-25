import { useCallback, useEffect, useRef } from 'react';
import { FILE_DIALOG_SETTLE_MAX_MS, FILE_DIALOG_SETTLE_MS } from '../config/constants';
import { browserDialogSettleEnv, createDialogSettleGate, waitForFileDialogToClose } from '../utils/fileDialogSettle';
import type { DialogSettleGate } from '../utils/fileDialogSettle';

const waitForDialog = () => waitForFileDialogToClose(browserDialogSettleEnv, { settleMs: FILE_DIALOG_SETTLE_MS, maxMs: FILE_DIALOG_SETTLE_MAX_MS });

/**
 * Runs `deliver` once the OS file dialog has closed, so its fade plays over a still screen instead of a screen
 * switch. A pending action is dropped if the component unmounts first.
 */
export const useAfterFileDialog = (): ((deliver: () => void) => void) => {
  const gateRef = useRef<DialogSettleGate | null>(null);
  useEffect(() => {
    // Built in the effect, not in a ref initialiser: StrictMode's dev double mount disposes once, and the gate must be live after the second mount.
    const gate = createDialogSettleGate(waitForDialog);
    gateRef.current = gate;
    return () => gate.dispose();
  }, []);
  return useCallback((deliver: () => void) => gateRef.current?.run(deliver), []);
};

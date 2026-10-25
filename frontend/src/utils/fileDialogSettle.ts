export interface DialogSettleEnv {
  hasFocus(): boolean;
  /** Registers a window-focus listener and returns its remover. */
  onFocus(listener: () => void): () => void;
  reducedMotion(): boolean;
}

export interface DialogSettleTiming { settleMs: number; maxMs: number; }

export interface DialogSettleGate {
  run(deliver: () => void): void;
  dispose(): void;
}

const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

export const browserDialogSettleEnv: DialogSettleEnv = {
  hasFocus: () => document.hasFocus(),
  onFocus: (listener) => {
    window.addEventListener('focus', listener);
    return () => window.removeEventListener('focus', listener);
  },
  reducedMotion: () => window.matchMedia(REDUCED_MOTION_QUERY).matches,
};

/**
 * Resolves once the OS has had time to play its file-dialog close animation: `settleMs` after the window last
 * (re)gained focus, or `maxMs` after the call if focus never comes. Reduced motion means no animation, so no wait.
 */
export const waitForFileDialogToClose = (env: DialogSettleEnv, timing: DialogSettleTiming): Promise<void> => new Promise((resolve) => {
  if (env.reducedMotion()) {
    resolve();
    return;
  }
  let settle: ReturnType<typeof setTimeout> | undefined;
  let cap: ReturnType<typeof setTimeout> | undefined;
  let removeFocusListener = () => {};
  const finish = () => {
    clearTimeout(settle);
    clearTimeout(cap);
    removeFocusListener();
    resolve();
  };
  const restartSettle = () => {
    clearTimeout(settle);
    settle = setTimeout(finish, timing.settleMs);
  };
  cap = setTimeout(finish, timing.maxMs);
  removeFocusListener = env.onFocus(restartSettle);
  if (env.hasFocus()) restartSettle();
});

/** Delivers one picked file after `wait`; ignores picks while one is pending and drops the action once disposed. */
export const createDialogSettleGate = (wait: () => Promise<void>): DialogSettleGate => {
  let pending = false;
  let disposed = false;
  return {
    run: (deliver) => {
      if (pending) return;
      pending = true;
      wait().then(() => {
        pending = false;
        if (!disposed) deliver();
      });
    },
    dispose: () => {
      disposed = true;
    },
  };
};

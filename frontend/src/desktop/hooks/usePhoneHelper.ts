import { useEffect, useMemo, useState } from 'react';
import type { DesktopCommands, PhoneHelperStatus } from '../commands/types';

export interface PhoneHelperActions {
  setEnabled: (enabled: boolean) => void;
  /** "Pair a phone", also "New code". */
  pairStart: () => void;
  pairCancel: () => void;
  forget: (peerId: string) => void;
  /** This computer's name. */
  rename: (name: string) => void;
  setDownloadVoices: (on: boolean) => void;
  setPort: (port: number) => void;
  dismissModelRequest: () => void;
  dismissVersionMismatch: () => void;
  allowThisNetwork: () => void;
}

export interface PhoneHelper {
  /** Null until the shell first answers. */
  status: PhoneHelperStatus | null;
  actions: PhoneHelperActions;
}

const report = (command: string) => (cause: unknown) => console.error(`Desktop command ${command} failed`, cause);

/** "Let my phone use this computer": the state from the shell (one read, then the `phone-helper` event) and the actions on it. */
export const usePhoneHelper = (commands: DesktopCommands): PhoneHelper => {
  const [status, setStatus] = useState<PhoneHelperStatus | null>(null);

  useEffect(() => {
    let live = true;
    let heardEvent = false;
    const stop = commands.onPhoneHelperStatus((next) => {
      heardEvent = true;
      setStatus(next);
    });
    // An event that came first is newer than this answer.
    commands.phoneHelperStatus().then((first) => {
      if (live && !heardEvent) setStatus(first);
    }).catch(report('phone_helper_status'));
    return () => {
      live = false;
      stop();
    };
  }, [commands]);

  const actions = useMemo((): PhoneHelperActions => ({
    setEnabled: (enabled) => {
      commands.phoneHelperSetEnabled(enabled).catch(report('phone_helper_set_enabled'));
    },
    pairStart: () => {
      commands.phoneHelperPairStart().catch(report('phone_helper_pair_start'));
    },
    pairCancel: () => {
      commands.phoneHelperPairCancel().catch(report('phone_helper_pair_cancel'));
    },
    forget: (peerId) => {
      commands.phoneHelperForget(peerId).catch(report('phone_helper_forget'));
    },
    rename: (name) => {
      commands.phoneHelperRename(name).catch(report('phone_helper_rename'));
    },
    setDownloadVoices: (on) => {
      commands.phoneHelperSetDownloadVoices(on).catch(report('phone_helper_set_download_voices'));
    },
    setPort: (port) => {
      commands.phoneHelperSetPort(port).catch(report('phone_helper_set_port'));
    },
    dismissModelRequest: () => {
      commands.phoneHelperDismissModelRequest().catch(report('phone_helper_dismiss_model_request'));
    },
    dismissVersionMismatch: () => {
      commands.phoneHelperDismissVersionMismatch().catch(report('phone_helper_dismiss_version_mismatch'));
    },
    allowThisNetwork: () => {
      commands.phoneHelperAllowThisNetwork().catch(report('phone_helper_allow_this_network'));
    },
  }), [commands]);

  return { status, actions };
};

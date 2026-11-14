import { useMemo } from 'react';
import type { DesktopCommands, PhoneHelperStatus } from '../commands/types';
import type { ReportCommandError } from './useCommandErrors';
import { useShellStatus } from './useShellStatus';
import type { ShellStatusSource } from './useShellStatus';

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
  /** The first `phone_helper_status` failed: the Phone tab offers `retryLoad`. */
  loadFailed: boolean;
  retryLoad: () => void;
  actions: PhoneHelperActions;
}

const PHONE_HELPER_STATUS: ShellStatusSource<PhoneHelperStatus> = {
  command: 'phone_helper_status',
  read: (commands) => commands.phoneHelperStatus(),
  subscribe: (commands, listener) => commands.onPhoneHelperStatus(listener),
};

/** "Let my phone use this computer": the state from the shell and the actions on it; a refused action goes to `report`. */
export const usePhoneHelper = (commands: DesktopCommands, report: ReportCommandError): PhoneHelper => {
  const { status, loadFailed, retry: retryLoad } = useShellStatus(commands, PHONE_HELPER_STATUS);

  const actions = useMemo((): PhoneHelperActions => ({
    setEnabled: (enabled) => {
      commands.phoneHelperSetEnabled(enabled).catch(report('phone_helper_set_enabled', enabled ? "Couldn't turn on phone help" : "Couldn't turn off phone help"));
    },
    pairStart: () => {
      commands.phoneHelperPairStart().catch(report('phone_helper_pair_start', "Couldn't start pairing"));
    },
    pairCancel: () => {
      commands.phoneHelperPairCancel().catch(report('phone_helper_pair_cancel', "Couldn't stop pairing"));
    },
    forget: (peerId) => {
      commands.phoneHelperForget(peerId).catch(report('phone_helper_forget', "Couldn't forget this phone"));
    },
    rename: (name) => {
      commands.phoneHelperRename(name).catch(report('phone_helper_rename', "Couldn't rename this computer"));
    },
    setDownloadVoices: (on) => {
      commands.phoneHelperSetDownloadVoices(on).catch(report('phone_helper_set_download_voices', "Couldn't change the voice download setting"));
    },
    setPort: (port) => {
      commands.phoneHelperSetPort(port).catch(report('phone_helper_set_port', "Couldn't change the port"));
    },
    dismissModelRequest: () => {
      commands.phoneHelperDismissModelRequest().catch(report('phone_helper_dismiss_model_request', "Couldn't close the voice request"));
    },
    dismissVersionMismatch: () => {
      commands.phoneHelperDismissVersionMismatch().catch(report('phone_helper_dismiss_version_mismatch', "Couldn't close the version notice"));
    },
    allowThisNetwork: () => {
      commands.phoneHelperAllowThisNetwork().catch(report('phone_helper_allow_this_network', "Couldn't add this network"));
    },
  }), [commands, report]);

  return { status, loadFailed, retryLoad, actions };
};

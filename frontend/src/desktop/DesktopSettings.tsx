import type { AppInfo, Licenses, LinkId, ModelStatus } from './commands/types';
import { AboutTab } from './components/SettingsDialog/AboutTab';
import { PhoneTabState } from './components/SettingsDialog/PhoneTabState';
import { SettingsDialog } from './components/SettingsDialog/SettingsDialog';
import type { SettingsTab } from './components/SettingsDialog/SettingsDialog';
import { VoiceModelTab } from './components/SettingsDialog/VoiceModelTab';
import type { ModelSetup } from './hooks/useModelSetup';
import type { PhoneHelper } from './hooks/usePhoneHelper';

interface DesktopSettingsProps {
  initialTab: SettingsTab;
  status: ModelStatus;
  setup: ModelSetup;
  phone: PhoneHelper;
  /** Null until the shell has answered; the tab stays empty until then. */
  info: AppInfo | null;
  licenses: Licenses | null;
  onOpenLink: (linkId: LinkId) => void;
  onClose: () => void;
}

/** The settings dialog with its three tabs wired to the shell's state. */
export const DesktopSettings = ({ initialTab, status, setup, phone, info, licenses, onOpenLink, onClose }: DesktopSettingsProps) => {
  const activeBundle = status.bundles.find((bundle) => bundle.active) ?? null;
  return (
    <SettingsDialog
      initialTab={initialTab}
      onClose={onClose}
      modelTab={(
        <VoiceModelTab
          status={status}
          exportingBookTitle={activeBundle?.usedByExports[0]?.title ?? null}
          refusedDelete={setup.refusedDelete}
          onDismissRefusedDelete={setup.dismissRefusedDelete}
          onDownload={setup.download}
          onPause={setup.pause}
          onCancel={setup.cancel}
          onActivate={setup.activate}
          onDelete={setup.deleteBundle}
          onImport={setup.importModel}
          importKind={setup.importKind}
        />
      )}
      phoneTab={<PhoneTabState phone={phone} />}
      aboutTab={info && <AboutTab info={info} licenses={licenses} onOpenLink={onOpenLink} />}
    />
  );
};

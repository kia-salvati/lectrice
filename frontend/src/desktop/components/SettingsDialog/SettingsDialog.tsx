import { useState } from 'react';
import { ModalDialog } from '../ModalDialog/ModalDialog';
import styles from './SettingsDialog.module.css';

export type SettingsTab = 'model' | 'phone' | 'about';

interface SettingsDialogProps {
  initialTab?: SettingsTab;
  onClose: () => void;
  /** The tabs' contents: <VoiceModelTab/>, <PhoneTab/> and <AboutTab/>. */
  modelTab: React.ReactNode;
  phoneTab: React.ReactNode;
  aboutTab: React.ReactNode;
}

const TAB_LABEL: Record<SettingsTab, string> = { model: 'Voice model', phone: 'Phone', about: 'About' };
const TABS: readonly SettingsTab[] = ['model', 'phone', 'about'];

/** UX2-C: a modal over whatever is playing, with a list of tabs on the left (pill tabs on top in a narrow window). */
export const SettingsDialog = ({ initialTab = 'model', onClose, modelTab, phoneTab, aboutTab }: SettingsDialogProps) => {
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const content: Record<SettingsTab, React.ReactNode> = { model: modelTab, phone: phoneTab, about: aboutTab };
  return (
    <ModalDialog title="Settings" size="wide" onClose={onClose}>
      <div className={styles.body}>
        <div className={styles.tabs} role="tablist" aria-label="Settings sections">
          {TABS.map((candidate) => (
            <button key={candidate} type="button" role="tab" className={styles.tab} aria-selected={tab === candidate} onClick={() => setTab(candidate)}>{TAB_LABEL[candidate]}</button>
          ))}
        </div>
        <div className={styles.content} role="tabpanel" aria-label={TAB_LABEL[tab]}>{content[tab]}</div>
      </div>
    </ModalDialog>
  );
};

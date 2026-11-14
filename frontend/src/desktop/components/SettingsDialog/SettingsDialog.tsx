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
const PANEL_ID = 'settings-panel';
const tabId = (tab: SettingsTab): string => `settings-tab-${tab}`;

/** Where each key takes the selection, from the selected tab's place in the list. Both arrow axes work: the list is a column when wide and a row when narrow. */
const KEY_MOVES: Partial<Record<string, (at: number) => number>> = {
  ArrowDown: (at) => (at + 1) % TABS.length,
  ArrowRight: (at) => (at + 1) % TABS.length,
  ArrowUp: (at) => (at - 1 + TABS.length) % TABS.length,
  ArrowLeft: (at) => (at - 1 + TABS.length) % TABS.length,
  Home: () => 0,
  End: () => TABS.length - 1,
};

/** UX2-C: a modal over whatever is playing, with a list of tabs on the left (pill tabs on top in a narrow window). */
export const SettingsDialog = ({ initialTab = 'model', onClose, modelTab, phoneTab, aboutTab }: SettingsDialogProps) => {
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const content: Record<SettingsTab, React.ReactNode> = { model: modelTab, phone: phoneTab, about: aboutTab };

  // The tab list is one Tab stop; the arrow keys move inside it, and the tab they land on is shown at once.
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const move = KEY_MOVES[event.key];
    if (move === undefined || event.ctrlKey || event.altKey || event.metaKey) return;
    event.preventDefault();
    const next = TABS[move(TABS.indexOf(tab))];
    setTab(next);
    event.currentTarget.querySelector<HTMLElement>(`#${tabId(next)}`)?.focus();
  };

  return (
    <ModalDialog title="Settings" size="wide" onClose={onClose}>
      <div className={styles.body}>
        <div className={styles.tabs} role="tablist" aria-label="Settings sections" aria-orientation="vertical" onKeyDown={handleKeyDown}>
          {TABS.map((candidate) => (
            <button key={candidate} type="button" role="tab" id={tabId(candidate)} className={styles.tab} aria-selected={tab === candidate} aria-controls={PANEL_ID} tabIndex={tab === candidate ? 0 : -1} onClick={() => setTab(candidate)}>{TAB_LABEL[candidate]}</button>
          ))}
        </div>
        <div className={styles.content} role="tabpanel" id={PANEL_ID} aria-labelledby={tabId(tab)}>{content[tab]}</div>
      </div>
    </ModalDialog>
  );
};

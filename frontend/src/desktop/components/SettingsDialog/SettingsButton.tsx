import desktop from '../../styles/controls.module.css';
import { GearIcon } from '../icons';

/** The library header's way into the settings dialog. */
export const SettingsButton = ({ onOpen }: { onOpen: () => void }) => (
  <button type="button" className={desktop.textButton} onClick={onOpen}><GearIcon />Settings</button>
);

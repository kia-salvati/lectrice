import styles from '../PhoneTab.module.css';

interface SwitchProps { id: string; checked: boolean; onChange: (checked: boolean) => void; }

export const Switch = ({ id, checked, onChange }: SwitchProps) => (
  <input type="checkbox" role="switch" id={id} className={styles.switch} checked={checked} onChange={(event) => onChange(event.target.checked)} />
);

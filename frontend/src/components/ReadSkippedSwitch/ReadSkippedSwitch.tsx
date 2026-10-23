interface ReadSkippedSwitchProps { checked: boolean; onChange: (checked: boolean) => void; }

export const ReadSkippedSwitch = ({ checked, onChange }: ReadSkippedSwitchProps) => (
  <label>
    <input type="checkbox" role="switch" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    <span>Read code &amp; tables aloud</span>
  </label>
);

import controls from '../../styles/controls.module.css';
import { formatVoiceOption } from '../../utils/voices';
import type { VoiceGroup } from '../../utils/voices';

interface VoicePickerProps { groups: VoiceGroup[]; value: string; available: boolean; onChange: (voiceId: string) => void; }

export const VoicePicker = ({ groups, value, available, onChange }: VoicePickerProps) => (
  <label>
    <span>Voice</span>
    <select className={controls.select} value={value} disabled={!available} onChange={(event) => onChange(event.target.value)}>
      {groups.map((group) => (
        <optgroup key={group.label} label={group.label}>
          {group.voices.map((voice) => <option key={voice.id} value={voice.id}>{formatVoiceOption(voice)}</option>)}
        </optgroup>
      ))}
    </select>
  </label>
);

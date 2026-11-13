import { Icon } from '../../components/icons/icons';

export const GearIcon = () => (
  <Icon size={18}>
    <circle cx="12" cy="12" r="3" fill="none" stroke="currentColor" strokeWidth="2" />
    <path fill="none" stroke="currentColor" strokeWidth="2" d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" />
  </Icon>
);

/** ↗: the link opens in the system browser. */
export const ExternalLinkIcon = () => <Icon size={14}><path fill="none" stroke="currentColor" strokeWidth="2" d="M14 4h6v6M20 4l-9 9M19 14v6H4V5h6" /></Icon>;

export const InfoIcon = () => (
  <Icon size={20}>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2" />
    <path stroke="currentColor" strokeWidth="2" d="M12 11v6" />
    <circle cx="12" cy="7.5" r="1.2" fill="currentColor" />
  </Icon>
);

export const WarningIcon = () => (
  <Icon size={20}>
    <path fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" d="M12 3l10 18H2z" />
    <path stroke="currentColor" strokeWidth="2" d="M12 10v5" />
    <circle cx="12" cy="18" r="1.2" fill="currentColor" />
  </Icon>
);

/** An outline handset. */
export const PhoneIcon = () => (
  <Icon size={16}>
    <rect x="7" y="2.75" width="10" height="18.5" rx="2" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <path stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" d="M10.75 18.25h2.5" />
  </Icon>
);

export const CopyIcon = () => (
  <Icon size={16}>
    <rect x="9" y="9" width="11" height="11" rx="2" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <path fill="none" stroke="currentColor" strokeWidth="1.5" d="M15 9V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h3" />
  </Icon>
);

/** The check of a finished step, larger than the inline one. */
export const DoneIcon = () => <Icon size={24}><path fill="none" stroke="currentColor" strokeWidth="2.2" d="M5 12l5 5 9-10" /></Icon>;

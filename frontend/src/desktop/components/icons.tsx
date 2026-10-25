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

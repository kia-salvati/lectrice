interface IconProps { size: number; children: React.ReactNode; }

const Icon = ({ size, children }: IconProps) => (
  <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">{children}</svg>
);

export const PrevIcon = () => <Icon size={20}><path fill="currentColor" d="M6 5h2v14H6zM20 5v14L9 12z" /></Icon>;

export const NextIcon = () => <Icon size={20}><path fill="currentColor" d="M16 5h2v14h-2zM4 5v14l11-7z" /></Icon>;

export const PlayIcon = () => <Icon size={22}><path fill="currentColor" d="M7 4v16l14-8z" /></Icon>;

export const PauseIcon = () => <Icon size={22}><path fill="currentColor" d="M6 4h4v16H6zM14 4h4v16h-4z" /></Icon>;

export const WaitIcon = () => <Icon size={22}><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" strokeWidth="2" strokeDasharray="14 8" /></Icon>;

export const MoreIcon = () => (
  <Icon size={20}>
    <circle cx="5" cy="12" r="2" fill="currentColor" />
    <circle cx="12" cy="12" r="2" fill="currentColor" />
    <circle cx="19" cy="12" r="2" fill="currentColor" />
  </Icon>
);

export const LocateIcon = () => <Icon size={20}><path fill="currentColor" d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm9 3h-2.07A7 7 0 0 0 13 5.07V3h-2v2.07A7 7 0 0 0 5.07 11H3v2h2.07A7 7 0 0 0 11 18.93V21h2v-2.07A7 7 0 0 0 18.93 13H21v-2zm-9 6a5 5 0 1 1 0-10 5 5 0 0 1 0 10z" /></Icon>;

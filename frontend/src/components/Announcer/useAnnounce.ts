import { createContext, useContext, useEffect } from 'react';

/** Says `text` to a screen reader; `urgent` interrupts what is being read. */
export type Announce = (text: string, urgent?: boolean) => void;

export const AnnounceContext = createContext<Announce>(() => {});

export const useAnnounce = (): Announce => useContext(AnnounceContext);

/** Says `text` when the calling component appears, and again whenever the text changes. */
export const useAnnounced = (text: string, urgent = false): void => {
  const announce = useAnnounce();
  useEffect(() => announce(text, urgent), [announce, text, urgent]);
};

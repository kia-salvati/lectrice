import { useCallback, useEffect, useRef, useState } from 'react';
import { NARROW_SCREEN_QUERY, RAIL_OPEN_STORAGE_KEY, WIDE_SCREEN_QUERY } from '../config/constants';
import { useMediaQuery } from './useMediaQuery';

export interface ChaptersPanel {
  /** The rail is on screen: wide window, the book has chapters, and the reader has not closed it. */
  railShown: boolean;
  /** The rail could show (wide window, chapters) but is closed: offer the reopen handle. */
  railClosed: boolean;
  isSheetOpen: boolean;
  /** Phones: the sheet is a modal. */
  isSheetModal: boolean;
  /** Bumped each time the rail should scroll to the current chapter and take focus. */
  railFocusRequest: number;
  openRail: () => void;
  closeRail: () => void;
  /** The popover's "Chapters…": the rail when it shows, the sheet otherwise. */
  openChapters: () => void;
  /** The pill's chapters button. */
  toggleSheet: () => void;
  closeSheet: () => void;
  /** The C key: the rail where it is available, the sheet elsewhere. */
  toggleWithKey: () => void;
  /** Escape inside a panel: back to where the reader was. */
  returnFocus: () => void;
}

const readRailOpen = (): boolean => {
  try {
    return localStorage.getItem(RAIL_OPEN_STORAGE_KEY) !== '0';
  } catch {
    return true; // storage blocked (private mode, file://): the rail simply starts open
  }
};

const writeRailOpen = (open: boolean): void => {
  try {
    localStorage.setItem(RAIL_OPEN_STORAGE_KEY, open ? '1' : '0');
  } catch {
    // storage blocked: the choice lasts for this page only
  }
};

/** View state of the chapters rail and sheet: which one is open, where focus came from, and what the C key does. */
export const useChaptersPanel = (hasToc: boolean): ChaptersPanel => {
  const isWide = useMediaQuery(WIDE_SCREEN_QUERY);
  const isSheetModal = useMediaQuery(NARROW_SCREEN_QUERY);
  const [isRailOpen, setIsRailOpen] = useState(readRailOpen);
  const [isSheetOpen, setIsSheetOpen] = useState(false);
  const [railFocusRequest, setRailFocusRequest] = useState(0);
  const openerRef = useRef<HTMLElement | null>(null);

  useEffect(() => writeRailOpen(isRailOpen), [isRailOpen]);

  const railAvailable = isWide && hasToc;
  const railShown = railAvailable && isRailOpen;

  const rememberOpener = () => {
    const active = document.activeElement;
    openerRef.current = active instanceof HTMLElement && active !== document.body ? active : null;
  };

  const returnFocus = useCallback(() => {
    const opener = openerRef.current;
    openerRef.current = null;
    if (opener?.isConnected) opener.focus();
    else if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  }, []);

  const openRail = () => {
    rememberOpener();
    setIsRailOpen(true);
    setRailFocusRequest((count) => count + 1);
  };

  const closeRail = () => {
    setIsRailOpen(false);
    returnFocus();
  };

  const openSheet = () => {
    rememberOpener();
    setIsSheetOpen(true);
  };

  const closeSheet = useCallback(() => {
    setIsSheetOpen(false);
    returnFocus();
  }, [returnFocus]);

  const openChapters = () => {
    if (!railShown) {
      openSheet();
      return;
    }
    rememberOpener();
    setRailFocusRequest((count) => count + 1);
  };

  const toggleSheet = () => (isSheetOpen ? closeSheet() : openSheet());

  const toggleWithKey = () => {
    if (isSheetOpen) closeSheet();
    else if (!railAvailable) openSheet();
    else if (isRailOpen) closeRail();
    else openRail();
  };

  return { railShown, railClosed: railAvailable && !isRailOpen, isSheetOpen, isSheetModal, railFocusRequest, openRail, closeRail, openChapters, toggleSheet, closeSheet, toggleWithKey, returnFocus };
};

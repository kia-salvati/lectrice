import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { ChevronIcon } from '../../../components/icons/icons';
import type { ModelImportKind } from '../../commands/types';
import desktop from '../../styles/controls.module.css';
import styles from './ImportMenu.module.css';

interface ImportMenuProps {
  /** "Import…" on the first-run screen, "Import a model…" in the settings. */
  label: string;
  /** A download or import is under way: the button is disabled and its title says why. */
  disabled: boolean;
  onImport: (kind: ModelImportKind) => void;
}

const ITEMS: readonly { kind: ModelImportKind; label: string }[] = [
  { kind: 'archive', label: 'Model file (.tar.bz2)…' },
  { kind: 'folder', label: 'Unpacked model folder…' },
];

const BUSY_TITLE = 'Wait for the download to finish, or cancel it.';
const MENU_GAP_PX = 4;

interface MenuPlace { top: number; left: number; }

/** Under the button, left edges aligned; above it when the window has no room below. */
const placeUnder = (button: HTMLElement, menuHeight: number): MenuPlace => {
  const rect = button.getBoundingClientRect();
  const roomBelow = window.innerHeight - rect.bottom - MENU_GAP_PX;
  const top = menuHeight <= roomBelow ? rect.bottom + MENU_GAP_PX : rect.top - MENU_GAP_PX - menuHeight;
  return { top, left: rect.left };
};

/** Where keyboard focus goes next in the menu, wrapping at both ends. */
const nextIndex = (items: readonly HTMLElement[], key: string): number | null => {
  const at = items.indexOf(document.activeElement as HTMLElement);
  switch (key) {
    case 'ArrowDown': return (at + 1) % items.length;
    case 'ArrowUp': return (at - 1 + items.length) % items.length;
    case 'Home': return 0;
    case 'End': return items.length - 1;
    default: return null;
  }
};

/**
 * S1-A: a menu button that offers the two ways to import a voice model. Enter, Space or ↓ open it and focus the first
 * item; ↑/↓ move; Escape closes and returns to the button; Tab or a click elsewhere closes. The menu is fixed so a
 * dialog's clipping never cuts it off.
 */
export const ImportMenu = ({ label, disabled, onImport }: ImportMenuProps) => {
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<MenuPlace | null>(null);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const buttonId = useId();

  const close = (focusButton: boolean) => {
    setOpen(false);
    setPlace(null);
    if (focusButton) buttonRef.current?.focus();
  };

  useLayoutEffect(() => {
    const button = buttonRef.current;
    const menu = menuRef.current;
    if (!open || !button || !menu) return;
    setPlace(placeUnder(button, menu.offsetHeight));
    (menu.querySelector('[role="menuitem"]') as HTMLElement | null)?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!wrapRef.current?.contains(event.target as Node)) close(false);
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open]);

  const onButtonKeyDown = (event: React.KeyboardEvent) => {
    if (event.key !== 'ArrowDown' || open) return;
    event.preventDefault();
    setOpen(true);
  };

  const onMenuKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      // Also keeps a surrounding <dialog> from taking the key as its own cancel.
      event.preventDefault();
      event.stopPropagation();
      close(true);
      return;
    }
    if (event.key === 'Tab') {
      // Focus goes back to the button first, so the Tab moves on from there instead of from a removed item.
      close(true);
      return;
    }
    const items = Array.from(menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
    const index = nextIndex(items, event.key);
    if (index === null) return;
    event.preventDefault();
    items[index]?.focus();
  };

  const pick = (kind: ModelImportKind) => {
    close(true);
    onImport(kind);
  };

  return (
    <span ref={wrapRef}>
      <button
        ref={buttonRef}
        id={buttonId}
        type="button"
        className={`${desktop.textButton} ${styles.button}`}
        disabled={disabled}
        title={disabled ? BUSY_TITLE : undefined}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => (open ? close(false) : setOpen(true))}
        onKeyDown={onButtonKeyDown}
      >
        {label}<span className={styles.chevron}><ChevronIcon /></span>
      </button>
      {open && (
        <div ref={menuRef} className={styles.menu} role="menu" aria-labelledby={buttonId} style={place ?? undefined} onKeyDown={onMenuKeyDown}>
          {ITEMS.map((item) => (
            <button key={item.kind} type="button" role="menuitem" tabIndex={-1} className={styles.item} onClick={() => pick(item.kind)}>{item.label}</button>
          ))}
        </div>
      )}
    </span>
  );
};

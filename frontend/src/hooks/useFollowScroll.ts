import { useCallback, useEffect, useRef, useState } from 'react';
import { startedInside } from '../utils/events';

/** Keeps the active sentence clear of the fixed pill at the bottom. */
const FOLLOW_BOTTOM_CLEARANCE_PX = 120;
/** Share of the window height above which the active sentence counts as scrolled out of view. */
const TOP_CLEARANCE_SHARE = 0.15;
/**
 * Smooth scrolling is for the sentence-by-sentence creep. A jump further than this many window heights (a chapter
 * click, a seek) is instant: a long smooth scroll through `content-visibility: auto` blocks drifts off its target as
 * their estimated heights are replaced by real ones along the way.
 */
const SMOOTH_SCROLL_MAX_SCREENS = 2;
const SCROLL_KEYS: ReadonlySet<string> = new Set(['PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown']);

export interface FollowScroll { isFollowing: boolean; resumeFollowing: () => void; }

const isFormField = (target: EventTarget | null): boolean => target instanceof Element && target.matches('input, select, textarea');

/** Scrolling or arrowing inside the chapters panels (`data-own-scroll`) moves them, not the text, so it is not "the reader scrolled". */
const isOwnScroll = (event: Event): boolean => startedInside(event, '[data-own-scroll]');

/** Cues and code lines have no element of their own, so the block is the fallback target. */
const findActiveElement = (sentenceId: number | null, blockIndex: number | null): Element | null => {
  const sentence = sentenceId === null ? null : document.querySelector(`[data-sentence-id="${sentenceId}"]`);
  if (sentence) return sentence;
  return blockIndex === null ? null : document.querySelector(`[data-block-index="${blockIndex}"]`);
};

/** View state: scrolls to the active sentence until the reader scrolls by hand. */
export const useFollowScroll = (activeSentenceId: number | null, activeBlockIndex: number | null): FollowScroll => {
  const [isFollowing, setIsFollowing] = useState(true);
  const hasScrolledRef = useRef(false);

  useEffect(() => {
    if (!isFollowing) return;
    const element = findActiveElement(activeSentenceId, activeBlockIndex);
    if (!element) return;
    const rect = element.getBoundingClientRect();
    const isAbove = rect.top < window.innerHeight * TOP_CLEARANCE_SHARE;
    const isBelow = rect.bottom > window.innerHeight - FOLLOW_BOTTOM_CLEARANCE_PX;
    if (isAbove || isBelow) {
      const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      const isNear = Math.abs(rect.top - window.innerHeight / 2) < window.innerHeight * SMOOTH_SCROLL_MAX_SCREENS;
      element.scrollIntoView({ block: 'center', behavior: hasScrolledRef.current && !reducedMotion && isNear ? 'smooth' : 'auto' });
    }
    hasScrolledRef.current = true;
  }, [activeSentenceId, activeBlockIndex, isFollowing]);

  useEffect(() => {
    const stopFollowing = () => setIsFollowing(false);
    const handleScrollGesture = (event: Event) => {
      if (!isOwnScroll(event)) stopFollowing();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (SCROLL_KEYS.has(event.key) && !isFormField(event.target) && !isOwnScroll(event)) stopFollowing();
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (event.target === document.documentElement) stopFollowing();
    };
    window.addEventListener('wheel', handleScrollGesture, { passive: true });
    window.addEventListener('touchmove', handleScrollGesture, { passive: true });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('pointerdown', handlePointerDown);
    return () => {
      window.removeEventListener('wheel', handleScrollGesture);
      window.removeEventListener('touchmove', handleScrollGesture);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('pointerdown', handlePointerDown);
    };
  }, []);

  const resumeFollowing = useCallback(() => setIsFollowing(true), []);

  return { isFollowing, resumeFollowing };
};

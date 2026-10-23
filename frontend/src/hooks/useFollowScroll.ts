import { useCallback, useEffect, useRef, useState } from 'react';

/** Keeps the active sentence clear of the fixed pill at the bottom. */
const FOLLOW_BOTTOM_CLEARANCE_PX = 120;
/** Share of the window height above which the active sentence counts as scrolled out of view. */
const TOP_CLEARANCE_SHARE = 0.15;
const SCROLL_KEYS: ReadonlySet<string> = new Set(['PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown']);

export interface FollowScroll { isFollowing: boolean; resumeFollowing: () => void; }

const isFormField = (target: EventTarget | null): boolean => target instanceof Element && target.matches('input, select, textarea');

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
      element.scrollIntoView({ block: 'center', behavior: hasScrolledRef.current && !reducedMotion ? 'smooth' : 'auto' });
    }
    hasScrolledRef.current = true;
  }, [activeSentenceId, activeBlockIndex, isFollowing]);

  useEffect(() => {
    const stopFollowing = () => setIsFollowing(false);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (SCROLL_KEYS.has(event.key) && !isFormField(event.target)) stopFollowing();
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (event.target === document.documentElement) stopFollowing();
    };
    window.addEventListener('wheel', stopFollowing, { passive: true });
    window.addEventListener('touchmove', stopFollowing, { passive: true });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('pointerdown', handlePointerDown);
    return () => {
      window.removeEventListener('wheel', stopFollowing);
      window.removeEventListener('touchmove', stopFollowing);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('pointerdown', handlePointerDown);
    };
  }, []);

  const resumeFollowing = useCallback(() => setIsFollowing(true), []);

  return { isFollowing, resumeFollowing };
};

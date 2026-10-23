/** Scrolls `container` so `element` sits in its middle. Instant: the panels open and jump without animation. */
export const centreIn = (container: HTMLElement, element: HTMLElement): void => {
  const box = container.getBoundingClientRect();
  const rect = element.getBoundingClientRect();
  container.scrollTop += rect.top - box.top - (box.height - rect.height) / 2;
};

/** Centres the element only if it has left the container's view. */
export const keepInView = (container: HTMLElement, element: HTMLElement): void => {
  const box = container.getBoundingClientRect();
  const rect = element.getBoundingClientRect();
  if (rect.top < box.top || rect.bottom > box.bottom) centreIn(container, element);
};

/** The title button of a toc entry inside a panel, if it is rendered. */
export const findTitleButton = (container: HTMLElement, index: number): HTMLButtonElement | null => container.querySelector<HTMLButtonElement>(`[data-toc-index="${index}"]`);

/** Every rendered title button, in reading order: what ↑ ↓ Home End move between. */
export const titleButtons = (container: HTMLElement): HTMLButtonElement[] => Array.from(container.querySelectorAll<HTMLButtonElement>('[data-toc-index]'));

/** The deepest entry of the path that is rendered (its ancestors are expanded), or null. */
export const findDeepestRendered = (container: HTMLElement, path: readonly number[]): HTMLButtonElement | null => {
  for (let k = path.length - 1; k >= 0; k -= 1) {
    const button = findTitleButton(container, path[k]);
    if (button) return button;
  }
  return null;
};

/** The title button of the row a key event happened in (the title itself or its disclosure button). */
export const titleOfRow = (target: EventTarget | null): HTMLButtonElement | null => (target instanceof Element ? target.closest('[data-toc-row]')?.querySelector<HTMLButtonElement>('[data-toc-index]') ?? null : null);

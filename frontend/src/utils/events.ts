/**
 * Whether the event started inside an element matching `selector`. Reads the event's path, not `target.closest`:
 * a React handler earlier on the path may have unmounted the target (a chapters row after a drill), and a detached
 * node no longer knows its ancestors.
 */
export const startedInside = (event: Event, selector: string): boolean => event.composedPath().some((node) => node instanceof Element && node.matches(selector));

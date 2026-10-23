import { useCallback, useEffect, useState } from 'react';
import { parseRoute, routeHref } from '../config/routes';
import type { Route } from '../config/routes';

/** The current screen from the URL hash, so Back and reload work without a router. */
export const useHashRoute = (): { route: Route; navigate: (route: Route) => void } => {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash));

  useEffect(() => {
    const handleHashChange = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  const navigate = useCallback((next: Route) => {
    window.location.hash = routeHref(next);
  }, []);

  return { route, navigate };
};

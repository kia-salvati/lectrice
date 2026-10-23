/** Toggles read once from the page URL, e.g. http://localhost:5173/?mock=noText,engineFailed#/ */
export type MockScenario = 'noText' | 'engineFailed' | 'offline' | 'empty' | 'audioError' | 'exportFailed' | 'exporterMissing';

const SCENARIOS: readonly MockScenario[] = ['noText', 'engineFailed', 'offline', 'empty', 'audioError', 'exportFailed', 'exporterMissing'];

const parseScenarios = (search: string): ReadonlySet<MockScenario> => {
  const requested = (new URLSearchParams(search).get('mock') ?? '').split(',').map((part) => part.trim());
  return new Set(SCENARIOS.filter((scenario) => requested.includes(scenario)));
};

const active = parseScenarios(window.location.search);

export const isScenarioActive = (scenario: MockScenario): boolean => active.has(scenario);

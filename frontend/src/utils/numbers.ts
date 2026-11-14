/** "1,284": counts as the screens show them. */
export const formatCount = (value: number): string => value.toLocaleString('en');

/** "34 %": a percent with a space before the sign, as in the Android app. */
export const formatPercent = (percent: number): string => `${percent} %`;

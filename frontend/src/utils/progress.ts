/** Share of a document read, 0–100, from the saved sentence id (ids are sequential from 0). Null if never started. */
export const percentRead = (lastSentenceId: number | null, sentenceCount: number): number | null => {
  if (lastSentenceId === null || sentenceCount <= 0) return null;
  return Math.min(100, Math.round(((lastSentenceId + 1) / sentenceCount) * 100));
};

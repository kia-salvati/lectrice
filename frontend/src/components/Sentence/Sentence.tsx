import type { Sentence as SentenceModel } from '../../types/document';
import styles from './Sentence.module.css';

export type SentenceState = 'unread' | 'read' | 'active' | 'activePreparing';

interface SentenceProps { sentence: SentenceModel; state: SentenceState; }

const STATE_CLASS: Record<SentenceState, string> = {
  unread: styles.sentence,
  read: `${styles.sentence} ${styles.read}`,
  active: `${styles.sentence} ${styles.active}`,
  activePreparing: `${styles.sentence} ${styles.active} ${styles.activePreparing}`,
};

const IS_ACTIVE: Record<SentenceState, boolean> = { unread: false, read: false, active: true, activePreparing: true };

export const Sentence = ({ sentence, state }: SentenceProps) => (
  <span data-sentence-id={sentence.id} className={STATE_CLASS[state]} aria-current={IS_ACTIVE[state] ? 'true' : undefined}>{sentence.text}</span>
);

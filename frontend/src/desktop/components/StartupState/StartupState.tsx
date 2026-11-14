import centered from '../../../styles/centered.module.css';
import controls from '../../../styles/controls.module.css';

interface StartupStateProps {
  /** The shell didn't answer the first voice-setup read. */
  failed: boolean;
  onRetry: () => void;
}

/** The window before the shell's first answer: a quiet line, or a way to ask again. */
export const StartupState = ({ failed, onRetry }: StartupStateProps) => {
  if (!failed) return <div className={centered.centered}><p className={centered.text} role="status">Starting Lectrice…</p></div>;
  return (
    <div className={centered.centered}>
      <h1 className={centered.title}>Lectrice couldn't start</h1>
      <p className={centered.text}>The voice setup didn't answer.</p>
      <div className={centered.actions}>
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={onRetry}>Try again</button>
      </div>
    </div>
  );
};

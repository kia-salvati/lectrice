import styles from './ProgressBar.module.css';

interface ProgressBarProps {
  /** 0–100; null means indeterminate (a sliding segment). */
  percent: number | null;
  /** "142 MB of 350 MB" for assistive tech; omitted while indeterminate. */
  valueText?: string;
  label: string;
  size?: 'big' | 'small';
}

export const ProgressBar = ({ percent, valueText, label, size = 'big' }: ProgressBarProps) => {
  const classes = [styles.track, size === 'small' ? styles.small : '', percent === null ? styles.indeterminate : ''].join(' ');
  const aria = percent === null ? {} : { 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': percent, 'aria-valuetext': valueText };
  return (
    <span className={classes} role="progressbar" aria-label={label} {...aria}>
      <span className={styles.fill} style={percent === null ? undefined : { width: `${percent}%` }} />
    </span>
  );
};

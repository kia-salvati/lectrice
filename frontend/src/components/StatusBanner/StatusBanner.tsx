import styles from './StatusBanner.module.css';

interface StatusBannerProps { message: string; actions?: React.ReactNode; }

/** A full-width error strip: message on the left, action buttons on the right. */
export const StatusBanner = ({ message, actions }: StatusBannerProps) => (
  <div className={styles.banner} role="alert">
    <span>{message}</span>
    {actions && <span className={styles.actions}>{actions}</span>}
  </div>
);

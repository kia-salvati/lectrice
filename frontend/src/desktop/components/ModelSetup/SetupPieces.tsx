import type { ModelBundle, ModelImportKind, ModelStatus, ModelTransfer } from '../../commands/types';
import desktop from '../../styles/controls.module.css';
import { formatMb, formatProgressMb, percentOf } from '../../utils/format';
import { importFileLabel } from '../../utils/importText';
import { INSTALL_SPARE_BYTES, installNeedsBytes, isTransferBusy } from '../../utils/modelSetup';
import { ImportMenu } from '../ImportMenu/ImportMenu';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import { BAR_LABEL } from './labels';
import styles from './ModelSetup.module.css';

type ProgressingTransfer = Extract<ModelTransfer, { doneBytes: number }>;

/** The setup column's "Import…" menu; it waits while a download or another import runs. */
export const ImportControl = ({ status, onImport }: { status: ModelStatus; onImport: (kind: ModelImportKind) => void }) => <ImportMenu label="Import…" disabled={isTransferBusy(status)} fileLabel={importFileLabel(status.bundles)} onImport={onImport} />;

export const CancelButton = ({ onCancel }: { onCancel: () => void }) => <button type="button" className={desktop.textButton} onClick={onCancel}>Cancel</button>;

export const Collapsed = ({ bundle }: { bundle: ModelBundle }) => <p className={styles.name}>{bundle.displayName} · {formatMb(bundle.downloadBytes)}</p>;

/** The progress block of a running download: the collapsed card (when the bundle is known), the bar and the lines under it. */
export const Progress = ({ bundle, transfer, children }: { bundle: ModelBundle | null; transfer: ProgressingTransfer | null; children: React.ReactNode }) => (
  <div className={styles.stack}>
    {bundle && <Collapsed bundle={bundle} />}
    <ProgressBar percent={transfer && percentOf(transfer.doneBytes, transfer.totalBytes)} valueText={transfer ? formatProgressMb(transfer.doneBytes, transfer.totalBytes) : undefined} label={BAR_LABEL} />
    <div className={styles.lines}>{children}</div>
  </div>
);

export const NoSpaceBox = ({ status, bundle, smaller }: { status: ModelStatus; bundle: ModelBundle | null; smaller: ModelBundle | null }) => (
  <>
    <NoticeBox kind="error">
      <p>
        {bundle === null
          ? 'Not enough space on this drive. Free up space, then try again.'
          : `Not enough space. ${bundle.displayName} needs ${formatMb(installNeedsBytes(bundle))} free on this drive while it installs (${formatMb(bundle.downloadBytes)} download, ${formatMb(bundle.installedBytes)} installed, ${formatMb(INSTALL_SPARE_BYTES)} to spare). There's ${formatMb(status.freeBytes ?? 0)} free now. Free up space, then try again.`}
      </p>
    </NoticeBox>
    {smaller && <p>{smaller.displayName} needs {formatMb(installNeedsBytes(smaller))} and fits.</p>}
  </>
);

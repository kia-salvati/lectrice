import { useState } from 'react';
import { CheckIcon, WaitIcon } from '../../../components/icons/icons';
import controls from '../../../styles/controls.module.css';
import type { ModelBundle, ModelBundleId, ModelImportKind, ModelStatus, ModelTransfer, TransferErrorName } from '../../commands/types';
import { IMPORT_TEXT, VARIANT_NOTE, VOICE_COUNT } from '../../config';
import desktop from '../../styles/controls.module.css';
import { formatApproxTimeLeft, formatMb, formatProgressMb, formatRetryIn, percentOf } from '../../utils/format';
import { installNeedsBytes, isTransferBusy } from '../../utils/modelSetup';
import { ImportMenu } from '../ImportMenu/ImportMenu';
import { ConfirmDialog, ModalDialog } from '../ModalDialog/ModalDialog';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import styles from './SettingsDialog.module.css';

export interface VoiceModelTabProps {
  status: ModelStatus;
  /** The book whose unfinished export uses the active model, if any: switching then asks first. */
  exportingBookTitle: string | null;
  /** The bundle whose deletion the shell refused (an unfinished export uses it); shown until dismissed. */
  refusedDelete: ModelBundleId | null;
  onDismissRefusedDelete: () => void;
  onDownload: (bundleId: ModelBundleId) => void;
  onPause: () => void;
  onCancel: () => void;
  onActivate: (bundleId: ModelBundleId) => void;
  onDelete: (bundleId: ModelBundleId) => void;
  /** From the import menu under the cards; also "Pick another file/folder…". */
  onImport: (kind: ModelImportKind) => void;
  /** Which kind the running or failed import asked for: the shell reports both as `source: 'import'`. */
  importKind: ModelImportKind;
}

type Confirm = { kind: 'switch'; to: ModelBundle; from: ModelBundle } | { kind: 'delete'; bundle: ModelBundle };

type RunningTransfer = Exclude<ModelTransfer, { kind: 'none' }>;
type ProgressingTransfer = Extract<ModelTransfer, { doneBytes: number }>;

const BAR_LABEL = 'Voice download';

const CancelButton = ({ onCancel }: Pick<VoiceModelTabProps, 'onCancel'>) => <button type="button" className={desktop.textButton} onClick={onCancel}>Cancel</button>;

/** One status line with the small bar under it, and the buttons that fit the moment. */
type ImportProps = Pick<VoiceModelTabProps, 'onImport' | 'importKind'>;

const TransferState = ({ transfer, bundle, status, onDownload, onPause, onCancel, onImport, importKind }: { transfer: RunningTransfer; bundle: ModelBundle; status: ModelStatus } & Pick<VoiceModelTabProps, 'onDownload' | 'onPause' | 'onCancel'> & ImportProps) => {
  const importText = IMPORT_TEXT[importKind];
  const bar = (progress: ProgressingTransfer | null) => (
    <span className={styles.bar}><ProgressBar percent={progress && percentOf(progress.doneBytes, progress.totalBytes)} valueText={progress ? formatProgressMb(progress.doneBytes, progress.totalBytes) : undefined} label={BAR_LABEL} size="small" /></span>
  );
  // The line and its bar stack; the buttons sit beside them when the card is wide enough.
  const progress = (line: string, progressing: ProgressingTransfer | null) => <div className={styles.progress}><span>{line}</span>{bar(progressing)}</div>;
  const pauseCancel = <div className={desktop.actions}><button type="button" className={controls.button} onClick={onPause}>Pause</button><CancelButton onCancel={onCancel} /></div>;
  const tryAgain = <div className={desktop.actions}><button type="button" className={controls.button} onClick={() => onDownload(bundle.id)}>Try again</button><CancelButton onCancel={onCancel} /></div>;
  const pickAnother = <div className={desktop.actions}><button type="button" className={controls.button} onClick={() => onImport(importKind)}>{importText.pickAnother}</button></div>;

  switch (transfer.kind) {
    case 'starting':
      return <>{progress('Starting…', null)}{pauseCancel}</>;
    case 'downloading': {
      const left = transfer.bytesPerSecond === null ? '' : ` · ${formatApproxTimeLeft((transfer.totalBytes - transfer.doneBytes) / transfer.bytesPerSecond)}`;
      return <>{progress(`${formatProgressMb(transfer.doneBytes, transfer.totalBytes)} · ${percentOf(transfer.doneBytes, transfer.totalBytes)} %${left}`, transfer)}{pauseCancel}</>;
    }
    case 'retrying':
      return <>{progress('Part of the download was damaged. Downloading it again…', transfer)}{pauseCancel}</>;
    case 'paused':
      return <>{progress(`Paused · ${formatProgressMb(transfer.doneBytes, transfer.totalBytes)}`, transfer)}<div className={desktop.actions}><button type="button" className={controls.button} onClick={() => onDownload(bundle.id)}>Resume</button><CancelButton onCancel={onCancel} /></div></>;
    case 'waitingForNetwork':
      return <>{progress(`No internet connection. Trying again in ${formatRetryIn(transfer.retryInSeconds)}`, transfer)}<div className={desktop.actions}><button type="button" className={controls.button} onClick={() => onDownload(bundle.id)}>Try now</button><CancelButton onCancel={onCancel} /></div></>;
    case 'verifying':
      return progress(transfer.source === 'import' ? importText[transfer.stage] : transfer.stage === 'unpacking' ? 'Unpacking…' : 'Checking the download…', null);
    case 'failed': {
      const noSpace = `Not enough space. ${bundle.displayName} needs ${formatMb(installNeedsBytes(bundle))} free on this drive; there's ${formatMb(status.freeBytes ?? 0)} free now.`;
      const LINE: Record<TransferErrorName, string> = {
        ModelChecksumMismatch: transfer.source === 'import' ? importText.notAModel : 'The download was damaged twice, so Lectrice stopped.',
        ModelDownloadFailed: 'The download failed. Try again in a moment.',
        FileUnreadable: importText.unreadable,
        StorageFull: noSpace,
      };
      return <><span className={desktop.errorText}>{LINE[transfer.error]}</span>{transfer.source === 'import' && transfer.error !== 'StorageFull' ? pickAnother : tryAgain}</>;
    }
  }
};

const ModelCard = (props: { bundle: ModelBundle; onUse: () => void; onAskDelete: () => void } & Pick<VoiceModelTabProps, 'status' | 'onDownload' | 'onPause' | 'onCancel'> & ImportProps) => {
  const { bundle, status } = props;
  const transfer = status.transfer.kind !== 'none' && status.transfer.bundleId === bundle.id ? status.transfer : null;
  let state: React.ReactNode;
  if (transfer) state = <TransferState transfer={transfer} bundle={bundle} status={status} onDownload={props.onDownload} onPause={props.onPause} onCancel={props.onCancel} onImport={props.onImport} importKind={props.importKind} />;
  else if (bundle.active && status.engine === 'loading') state = <span className={desktop.actions}><span className={`${desktop.ok} ${desktop.spin}`}><WaitIcon /></span><span>Switching… The voice restarts in a few seconds.</span></span>;
  else if (bundle.active) state = <span className={desktop.ok}><CheckIcon /><span>In use · {formatMb(bundle.installedBytes)} on disk</span></span>;
  else if (bundle.installed) {
    state = (
      <>
        <span>Installed · {formatMb(bundle.installedBytes)} on disk</span>
        <div className={desktop.actions}>
          <button type="button" className={controls.button} onClick={props.onUse}>Use this</button>
          <button type="button" className={`${desktop.textButton} ${desktop.danger}`} onClick={props.onAskDelete}>Delete…</button>
        </div>
      </>
    );
  } else {
    state = (
      <>
        <span>Not downloaded · {formatMb(bundle.downloadBytes)} download</span>
        <div className={desktop.actions}><button type="button" className={controls.button} disabled={isTransferBusy(status)} onClick={() => props.onDownload(bundle.id)}>Download {formatMb(bundle.downloadBytes)}</button></div>
      </>
    );
  }
  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <span className={styles.name}>{bundle.displayName}</span>
        <span className={styles.sizes}>{formatMb(bundle.downloadBytes)} download · {formatMb(bundle.installedBytes)} on disk</span>
      </div>
      <p className={desktop.muted}>{VARIANT_NOTE[bundle.variant]}</p>
      <div className={styles.state}>{state}</div>
    </div>
  );
};

/** An import that hasn't matched a bundle yet has no card to live in; it shows under them. */
const UnmatchedImport = ({ transfer, importKind }: { transfer: ModelTransfer } & Pick<VoiceModelTabProps, 'importKind'>) => {
  if (transfer.kind === 'none' || transfer.bundleId !== null || transfer.source !== 'import') return null;
  const importText = IMPORT_TEXT[importKind];
  if (transfer.kind === 'verifying') return <div className={styles.progress}><span>{importText[transfer.stage]}</span><span className={styles.bar}><ProgressBar percent={null} label="Voice import" size="small" /></span></div>;
  if (transfer.kind === 'failed') return <span className={desktop.errorText}>{transfer.error === 'ModelChecksumMismatch' ? importText.notAModel : importText.unreadable}</span>;
  return null;
};

/** The Voice model tab: both bundles as cards, import, and the switch / delete questions. */
export const VoiceModelTab = (props: VoiceModelTabProps) => {
  const { status } = props;
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const active = status.bundles.find((bundle) => bundle.active) ?? null;
  const refused = status.bundles.find((bundle) => bundle.id === props.refusedDelete) ?? null;
  const busy = isTransferBusy(status);

  const use = (bundle: ModelBundle) => {
    if (props.exportingBookTitle !== null && active !== null) setConfirm({ kind: 'switch', to: bundle, from: active });
    else props.onActivate(bundle.id);
  };
  const confirmed = () => {
    if (confirm?.kind === 'switch') props.onActivate(confirm.to.id);
    if (confirm?.kind === 'delete') props.onDelete(confirm.bundle.id);
    setConfirm(null);
  };

  return (
    <div className={styles.stack}>
      {status.bundles.map((bundle) => (
        <ModelCard key={bundle.id} bundle={bundle} status={status} onUse={() => use(bundle)} onAskDelete={() => setConfirm({ kind: 'delete', bundle })} onDownload={props.onDownload} onPause={props.onPause} onCancel={props.onCancel} onImport={props.onImport} importKind={props.importKind} />
      ))}
      <UnmatchedImport transfer={status.transfer} importKind={props.importKind} />
      <div className={desktop.actions}>
        <ImportMenu label="Import a model…" disabled={busy} onImport={props.onImport} />
      </div>
      <p className={desktop.muted}>Both models have the same {VOICE_COUNT} English voices.</p>

      {confirm?.kind === 'switch' && (
        <ConfirmDialog title={`Switch to ${confirm.to.displayName}?`} text={`“${props.exportingBookTitle}” is being exported with ${confirm.from.displayName}. It will wait until ${confirm.from.displayName} is in use again.`} cancelLabel="Cancel" confirmLabel="Switch" onCancel={() => setConfirm(null)} onConfirm={confirmed} />
      )}
      {confirm?.kind === 'delete' && (
        <ConfirmDialog title={`Delete the ${confirm.bundle.displayName} model?`} text={`Frees ${formatMb(confirm.bundle.installedBytes)}. You can download it again later.`} cancelLabel="Cancel" confirmLabel="Delete" danger onCancel={() => setConfirm(null)} onConfirm={confirmed} />
      )}
      {refused && (
        <ModalDialog title="An export uses this model" showClose={false} onClose={props.onDismissRefusedDelete} footer={<button type="button" className={`${controls.button} ${controls.primary}`} onClick={props.onDismissRefusedDelete}>OK</button>}>
          <p>Cancel that export first, or wait until it's done.</p>
        </ModalDialog>
      )}
    </div>
  );
};

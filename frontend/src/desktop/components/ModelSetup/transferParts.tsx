import controls from '../../../styles/controls.module.css';
import type { ModelTransfer, TransferErrorName, TransferSource } from '../../commands/types';
import desktop from '../../styles/controls.module.css';
import { formatApproxTimeLeft, formatProgressMb, formatRetryIn, formatSpeed, percentOf } from '../../utils/format';
import { importText } from '../../utils/importText';
import { bundleById, smallerThatFits } from '../../utils/modelSetup';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import { BAR_LABEL, DOWNLOADING_HEADING } from './labels';
import type { ModelSetupProps } from './ModelSetup';
import styles from './ModelSetup.module.css';
import { CancelButton, Collapsed, ImportControl, NoSpaceBox, Progress } from './SetupPieces';

/** Heading, main block and action buttons; the column renders them in that order. */
export interface SetupParts { heading?: string; main: React.ReactNode; actions?: React.ReactNode; }

type RunningTransfer = Exclude<ModelTransfer, { kind: 'none' }>;
type FailedTransfer = Extract<ModelTransfer, { kind: 'failed' }>;

export const primary = `${controls.button} ${controls.primary}`;

const failedParts = (props: ModelSetupProps, transfer: FailedTransfer): SetupParts => {
  const bundle = bundleById(props.status, transfer.bundleId);
  const smaller = smallerThatFits(props.status, bundle);
  const copy = importText(props.importKind, props.status.bundles);
  const importAgain = () => props.onImport(props.importKind);
  const tryAgain = <button type="button" className={primary} onClick={bundle === null ? importAgain : () => props.onDownload(bundle.id)}>Try again</button>;
  const pickAnother = <button type="button" className={primary} onClick={importAgain}>{copy.pickAnother}</button>;
  const collapsed = bundle && <Collapsed bundle={bundle} />;
  const downloadFailed: SetupParts = { heading: DOWNLOADING_HEADING, main: <>{collapsed}<NoticeBox kind="error"><p>The download failed. Try again in a moment.</p></NoticeBox></>, actions: tryAgain };
  const unreadable: SetupParts = { main: <NoticeBox kind="error"><p>{copy.unreadable}</p></NoticeBox>, actions: pickAnother };
  const noSpace = (heading?: string): SetupParts => ({
    heading,
    main: <>{collapsed}<NoSpaceBox status={props.status} bundle={bundle} smaller={smaller} /></>,
    actions: <>{tryAgain}{smaller && <button type="button" className={controls.button} onClick={() => props.onDownload(smaller.id)}>Download {smaller.displayName} instead</button>}</>,
  });

  const BY_ERROR: Record<TransferSource, Record<TransferErrorName, SetupParts>> = {
    download: {
      ModelChecksumMismatch: { heading: DOWNLOADING_HEADING, main: <>{collapsed}<NoticeBox kind="error"><p>The download was damaged twice, so Lectrice stopped. Check your connection and try again.</p></NoticeBox></>, actions: <>{tryAgain}<ImportControl status={props.status} onImport={props.onImport} /></> },
      ModelDownloadFailed: downloadFailed,
      FileUnreadable: downloadFailed,
      StorageFull: noSpace(DOWNLOADING_HEADING),
    },
    import: {
      ModelChecksumMismatch: {
        main: <NoticeBox kind="error"><p>{copy.notAModel}</p></NoticeBox>,
        actions: <>{pickAnother}<button type="button" className={desktop.textButton} onClick={() => props.onOpenLink('modelDownloads')}>Where do I get it?</button></>,
      },
      FileUnreadable: unreadable,
      ModelDownloadFailed: unreadable,
      StorageFull: noSpace(),
    },
  };
  return BY_ERROR[transfer.source][transfer.error];
};

/** The setup column while a download or an import runs, waits or has failed. */
export const transferParts = (props: ModelSetupProps, transfer: RunningTransfer): SetupParts => {
  const pauseButton = <button type="button" className={controls.button} onClick={props.onPause}>Pause</button>;
  const cancelButton = <CancelButton onCancel={props.onCancel} />;
  switch (transfer.kind) {
    case 'starting':
      return { heading: DOWNLOADING_HEADING, main: <Progress bundle={bundleById(props.status, transfer.bundleId)} transfer={null}><p>Starting…</p></Progress>, actions: <>{pauseButton}{cancelButton}</> };
    case 'downloading': {
      const progressText = `${formatProgressMb(transfer.doneBytes, transfer.totalBytes)} · ${percentOf(transfer.doneBytes, transfer.totalBytes)} %`;
      const speedText = transfer.bytesPerSecond === null ? null : `${formatSpeed(transfer.bytesPerSecond)} · ${formatApproxTimeLeft((transfer.totalBytes - transfer.doneBytes) / transfer.bytesPerSecond)}`;
      return {
        heading: DOWNLOADING_HEADING,
        main: (
          <Progress bundle={bundleById(props.status, transfer.bundleId)} transfer={transfer}>
            <p>{progressText}</p>
            {speedText && <p className={desktop.muted}>{speedText}</p>}
            <p className={desktop.muted}>You can open a PDF meanwhile. The download keeps going.</p>
          </Progress>
        ),
        actions: (
          <>
            {pauseButton}
            <div className={`${desktop.actions} ${desktop.centered}`}>
              {cancelButton}
              <ImportControl status={props.status} onImport={props.onImport} />
              <button type="button" className={desktop.textButton} onClick={props.onGoToLibrary}>Meanwhile, open a PDF</button>
            </div>
          </>
        ),
      };
    }
    case 'retrying':
      return { heading: DOWNLOADING_HEADING, main: <Progress bundle={bundleById(props.status, transfer.bundleId)} transfer={transfer}><p>Part of the download was damaged. Downloading it again…</p></Progress>, actions: <>{pauseButton}{cancelButton}</> };
    case 'paused':
      return {
        heading: DOWNLOADING_HEADING,
        main: <Progress bundle={bundleById(props.status, transfer.bundleId)} transfer={transfer}><p>Paused · {formatProgressMb(transfer.doneBytes, transfer.totalBytes)}</p></Progress>,
        actions: <><button type="button" className={primary} onClick={() => props.onDownload(transfer.bundleId)}>Resume</button>{cancelButton}</>,
      };
    case 'waitingForNetwork':
      return {
        heading: DOWNLOADING_HEADING,
        main: (
          <Progress bundle={bundleById(props.status, transfer.bundleId)} transfer={transfer}>
            <p>No internet connection. The download continues by itself when you're back online.</p>
            <p className={desktop.muted}>Trying again in {formatRetryIn(transfer.retryInSeconds)}</p>
          </Progress>
        ),
        actions: <><button type="button" className={controls.button} onClick={() => props.onDownload(transfer.bundleId)}>Try now</button>{cancelButton}</>,
      };
    case 'verifying': {
      const bundle = bundleById(props.status, transfer.bundleId);
      const text = transfer.source === 'import' ? importText(props.importKind, props.status.bundles)[transfer.stage] : transfer.stage === 'unpacking' ? 'Unpacking…' : 'Checking the download…';
      return {
        heading: transfer.source === 'download' ? DOWNLOADING_HEADING : undefined,
        main: (
          <div className={styles.stack}>
            {bundle && transfer.source === 'download' && <Collapsed bundle={bundle} />}
            <p>{text}</p>
            <ProgressBar percent={null} label={BAR_LABEL} />
          </div>
        ),
      };
    }
    case 'failed':
      return failedParts(props, transfer);
  }
};

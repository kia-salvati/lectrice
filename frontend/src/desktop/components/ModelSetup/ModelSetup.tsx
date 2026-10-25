import { LibraryHeader } from '../../../components/LibraryHeader/LibraryHeader';
import controls from '../../../styles/controls.module.css';
import type { LinkId, ModelBundle, ModelBundleId, ModelImportKind, ModelStatus, ModelTransfer, TransferErrorName, TransferSource } from '../../commands/types';
import { IMPORT_TEXT, VARIANT_NOTE, VOICE_COUNT } from '../../config';
import desktop from '../../styles/controls.module.css';
import { formatApproxTimeLeft, formatMb, formatProgressMb, formatRetryIn, formatSpace, formatSpeed, percentOf } from '../../utils/format';
import { INSTALL_SPARE_BYTES, bundleById, installNeedsBytes, installPhase, isTransferBusy, smallerThatFits } from '../../utils/modelSetup';
import type { InstallPhase } from '../../utils/modelSetup';
import { ImportMenu } from '../ImportMenu/ImportMenu';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { ProgressBar } from '../ProgressBar/ProgressBar';
import { SampleCard } from '../SampleCard/SampleCard';
import type { SampleState } from '../SampleCard/SampleCard';
import styles from './ModelSetup.module.css';

export interface ModelSetupProps {
  status: ModelStatus;
  selectedBundleId: ModelBundleId | null;
  onSelect: (bundleId: ModelBundleId) => void;
  /** Download, Resume, Try now, Try again and "Download Smaller instead" all (re)start a bundle's download. */
  onDownload: (bundleId: ModelBundleId) => void;
  onPause: () => void;
  onCancel: () => void;
  /** "Model file (.tar.bz2)…" or "Unpacked model folder…" from the import menu; also "Pick another file/folder…". */
  onImport: (kind: ModelImportKind) => void;
  /** Which kind the running or failed import asked for: the shell reports both as `source: 'import'`. */
  importKind: ModelImportKind;
  onNotNow: () => void;
  /** "Meanwhile, open a PDF" and "Go to library": the library, whose strip follows the download. */
  onGoToLibrary: () => void;
  onOpenPdf: () => void;
  /** The voice couldn't start: load it again. */
  onRetryEngine: () => void;
  /** Back to the choice after a failed start. */
  onDownloadAgain: () => void;
  onOpenLink: (linkId: LinkId) => void;
  /** "Heart · American English" */
  voiceLabel: string;
  sample: SampleState;
  onToggleSample: () => void;
}

const INTRO = "Lectrice reads with a natural voice that runs on this computer. It's a one-time download. After that everything works offline, and your books never leave this computer.";
const DOWNLOADING_HEADING = 'Downloading the voice';
const BAR_LABEL = 'Voice download';

/** Heading, main block and action buttons; the column renders them in that order. */
interface SetupParts { heading?: string; main: React.ReactNode; actions?: React.ReactNode; }

type RunningTransfer = Exclude<ModelTransfer, { kind: 'none' }>;
type FailedTransfer = Extract<ModelTransfer, { kind: 'failed' }>;
type ProgressingTransfer = Extract<ModelTransfer, { doneBytes: number }>;

const primary = `${controls.button} ${controls.primary}`;

const RadioCard = ({ bundle, checked, onSelect }: { bundle: ModelBundle; checked: boolean; onSelect: () => void }) => (
  <label className={styles.card}>
    <input type="radio" name="model" value={bundle.id} checked={checked} onChange={onSelect} />
    <span className={styles.name}>{bundle.displayName}</span>
    <span className={styles.sizes}>{formatMb(bundle.downloadBytes)} download · {formatMb(bundle.installedBytes)} on disk</span>
    <span className={styles.note}>{VARIANT_NOTE[bundle.variant]}</span>
  </label>
);

const ImportControl = ({ status, onImport }: Pick<ModelSetupProps, 'status' | 'onImport'>) => <ImportMenu label="Import…" disabled={isTransferBusy(status)} onImport={onImport} />;

const CancelButton = ({ onCancel }: { onCancel: () => void }) => <button type="button" className={desktop.textButton} onClick={onCancel}>Cancel</button>;

const Collapsed = ({ bundle }: { bundle: ModelBundle }) => <p className={styles.name}>{bundle.displayName} · {formatMb(bundle.downloadBytes)}</p>;

/** The progress block of a running download: the collapsed card, the bar and the lines under it. */
const Progress = ({ bundle, transfer, children }: { bundle: ModelBundle; transfer: ProgressingTransfer | null; children: React.ReactNode }) => (
  <div className={styles.stack}>
    <Collapsed bundle={bundle} />
    <ProgressBar percent={transfer && percentOf(transfer.doneBytes, transfer.totalBytes)} valueText={transfer ? formatProgressMb(transfer.doneBytes, transfer.totalBytes) : undefined} label={BAR_LABEL} />
    <div className={styles.lines}>{children}</div>
  </div>
);

const choosingParts = (props: ModelSetupProps): SetupParts => {
  const { status, selectedBundleId } = props;
  const selected = bundleById(status, selectedBundleId);
  return {
    heading: 'Choose a voice to get started',
    main: (
      <>
        <p>{INTRO}</p>
        <div className={styles.cards} role="radiogroup" aria-label="Model">
          {status.bundles.map((bundle) => <RadioCard key={bundle.id} bundle={bundle} checked={bundle.id === selectedBundleId} onSelect={() => props.onSelect(bundle.id)} />)}
        </div>
        <div className={styles.stack}>
          <p>Not sure? Pick Full quality.</p>
          <p>Both have the same {VOICE_COUNT} English voices. You can switch later in Settings.</p>
          {status.freeBytes !== null && <p className={desktop.muted}>Free space on this drive: {formatSpace(status.freeBytes)}</p>}
        </div>
      </>
    ),
    actions: (
      <>
        {selected
          ? <button type="button" className={primary} onClick={() => props.onDownload(selected.id)}>Download {formatMb(selected.downloadBytes)}</button>
          : <button type="button" className={primary} disabled>Choose a model</button>}
        <div className={`${desktop.actions} ${desktop.centered}`}>
          <ImportControl status={status} onImport={props.onImport} />
          <button type="button" className={desktop.textButton} onClick={props.onNotNow}>Not now, just read</button>
        </div>
      </>
    ),
  };
};

const NoSpaceBox = ({ status, bundle, smaller }: { status: ModelStatus; bundle: ModelBundle | null; smaller: ModelBundle | null }) => (
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

const failedParts = (props: ModelSetupProps, transfer: FailedTransfer): SetupParts => {
  const bundle = bundleById(props.status, transfer.bundleId);
  const smaller = smallerThatFits(props.status, bundle);
  const importText = IMPORT_TEXT[props.importKind];
  const importAgain = () => props.onImport(props.importKind);
  const tryAgain = <button type="button" className={primary} onClick={bundle === null ? importAgain : () => props.onDownload(bundle.id)}>Try again</button>;
  const pickAnother = <button type="button" className={primary} onClick={importAgain}>{importText.pickAnother}</button>;
  const collapsed = bundle && <Collapsed bundle={bundle} />;
  const downloadFailed: SetupParts = { heading: DOWNLOADING_HEADING, main: <>{collapsed}<NoticeBox kind="error"><p>The download failed. Try again in a moment.</p></NoticeBox></>, actions: tryAgain };
  const unreadable: SetupParts = { main: <NoticeBox kind="error"><p>{importText.unreadable}</p></NoticeBox>, actions: pickAnother };
  const noSpace = (heading?: string): SetupParts => ({
    heading,
    main: <>{collapsed}<NoSpaceBox status={props.status} bundle={bundle} smaller={smaller} /></>,
    actions: <>{tryAgain}{smaller && <button type="button" className={controls.button} onClick={() => props.onDownload(smaller.id)}>Download Smaller instead</button>}</>,
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
        main: <NoticeBox kind="error"><p>{importText.notAModel}</p></NoticeBox>,
        actions: <>{pickAnother}<button type="button" className={desktop.textButton} onClick={() => props.onOpenLink('modelDownloads')}>Where do I get it?</button></>,
      },
      FileUnreadable: unreadable,
      ModelDownloadFailed: unreadable,
      StorageFull: noSpace(),
    },
  };
  return BY_ERROR[transfer.source][transfer.error];
};

const transferParts = (props: ModelSetupProps, transfer: RunningTransfer): SetupParts => {
  const pauseButton = <button type="button" className={controls.button} onClick={props.onPause}>Pause</button>;
  const cancelButton = <CancelButton onCancel={props.onCancel} />;
  switch (transfer.kind) {
    case 'starting':
      return { heading: DOWNLOADING_HEADING, main: <Progress bundle={bundleById(props.status, transfer.bundleId)!} transfer={null}><p>Starting…</p></Progress>, actions: <>{pauseButton}{cancelButton}</> };
    case 'downloading': {
      const progressText = `${formatProgressMb(transfer.doneBytes, transfer.totalBytes)} · ${percentOf(transfer.doneBytes, transfer.totalBytes)} %`;
      const speedText = transfer.bytesPerSecond === null ? null : `${formatSpeed(transfer.bytesPerSecond)} · ${formatApproxTimeLeft((transfer.totalBytes - transfer.doneBytes) / transfer.bytesPerSecond)}`;
      return {
        heading: DOWNLOADING_HEADING,
        main: (
          <Progress bundle={bundleById(props.status, transfer.bundleId)!} transfer={transfer}>
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
      return { heading: DOWNLOADING_HEADING, main: <Progress bundle={bundleById(props.status, transfer.bundleId)!} transfer={transfer}><p>Part of the download was damaged. Downloading it again…</p></Progress>, actions: <>{pauseButton}{cancelButton}</> };
    case 'paused':
      return {
        heading: DOWNLOADING_HEADING,
        main: <Progress bundle={bundleById(props.status, transfer.bundleId)!} transfer={transfer}><p>Paused · {formatProgressMb(transfer.doneBytes, transfer.totalBytes)}</p></Progress>,
        actions: <><button type="button" className={primary} onClick={() => props.onDownload(transfer.bundleId)}>Resume</button>{cancelButton}</>,
      };
    case 'waitingForNetwork':
      return {
        heading: DOWNLOADING_HEADING,
        main: (
          <Progress bundle={bundleById(props.status, transfer.bundleId)!} transfer={transfer}>
            <p>No internet connection. The download continues by itself when you're back online.</p>
            <p className={desktop.muted}>Trying again in {formatRetryIn(transfer.retryInSeconds)}</p>
          </Progress>
        ),
        actions: <><button type="button" className={controls.button} onClick={() => props.onDownload(transfer.bundleId)}>Try now</button>{cancelButton}</>,
      };
    case 'verifying': {
      const bundle = bundleById(props.status, transfer.bundleId);
      const text = transfer.source === 'import' ? IMPORT_TEXT[props.importKind][transfer.stage] : transfer.stage === 'unpacking' ? 'Unpacking…' : 'Checking the download…';
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

const PHASE_PARTS: Record<InstallPhase, (props: ModelSetupProps) => SetupParts> = {
  choosing: choosingParts,
  engineLoading: (props) => ({ heading: 'Starting the voice…', main: <SampleCard voiceLabel={props.voiceLabel} state="engineLoading" onToggle={props.onToggleSample} /> }),
  engineFailed: (props) => ({
    heading: DOWNLOADING_HEADING,
    main: <NoticeBox kind="error"><p>The voice couldn't start. Try again, or download it again.</p></NoticeBox>,
    actions: <><button type="button" className={primary} onClick={props.onRetryEngine}>Try again</button><button type="button" className={desktop.textButton} onClick={props.onDownloadAgain}>Download again</button></>,
  }),
  ready: (props) => ({
    heading: 'The voice is ready',
    main: <SampleCard voiceLabel={props.voiceLabel} state={props.sample} onToggle={props.onToggleSample} />,
    actions: <><button type="button" className={primary} onClick={props.onOpenPdf}>Open a PDF…</button><button type="button" className={controls.button} onClick={props.onGoToLibrary}>Go to library</button></>,
  }),
};

/** UX1-A: one centred column that changes in place from the choice through the download to the ready sample. */
export const ModelSetup = (props: ModelSetupProps) => {
  const { transfer } = props.status;
  const parts = transfer.kind === 'none' ? PHASE_PARTS[installPhase(props.status)](props) : transferParts(props, transfer);
  return (
    <section className={styles.setup} aria-label="Voice setup" aria-busy={isTransferBusy(props.status)}>
      {parts.heading && <h2 className={styles.heading}>{parts.heading}</h2>}
      {parts.main}
      {parts.actions && <div className={styles.column}>{parts.actions}</div>}
    </section>
  );
};

/** The first-run screen: the library header over the setup column. */
export const FirstRunPage = (props: ModelSetupProps) => (
  <div className={styles.page}>
    <LibraryHeader />
    <ModelSetup {...props} />
  </div>
);

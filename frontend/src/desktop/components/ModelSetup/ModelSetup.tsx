import { LibraryHeader } from '../../../components/LibraryHeader/LibraryHeader';
import controls from '../../../styles/controls.module.css';
import type { LinkId, ModelBundle, ModelBundleId, ModelImportKind, ModelStatus } from '../../commands/types';
import { RECOMMENDED_VARIANT, VARIANT_NOTE, VOICE_COUNT } from '../../config';
import desktop from '../../styles/controls.module.css';
import { formatMb, formatSpace } from '../../utils/format';
import { bundleById, installPhase, isTransferBusy } from '../../utils/modelSetup';
import type { InstallPhase } from '../../utils/modelSetup';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { SampleCard } from '../SampleCard/SampleCard';
import type { SampleState } from '../SampleCard/SampleCard';
import { DOWNLOADING_HEADING } from './labels';
import styles from './ModelSetup.module.css';
import { ImportControl } from './SetupPieces';
import { primary, transferParts } from './transferParts';
import type { SetupParts } from './transferParts';

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
  /** The last try to play the sample failed. */
  sampleFailed: boolean;
  onToggleSample: () => void;
}

const INTRO = "Lectrice reads with a natural voice that runs on this computer. It's a one-time download. After that everything works offline, and your books never leave this computer.";
const RadioCard = ({ bundle, checked, onSelect }: { bundle: ModelBundle; checked: boolean; onSelect: () => void }) => (
  <label className={styles.card}>
    <input type="radio" name="model" value={bundle.id} checked={checked} onChange={onSelect} />
    <span className={styles.name}>{bundle.displayName}</span>
    <span className={styles.sizes}>{formatMb(bundle.downloadBytes)} download · {formatMb(bundle.installedBytes)} on disk</span>
    <span className={styles.note}>{VARIANT_NOTE[bundle.variant]}</span>
  </label>
);

const choosingParts = (props: ModelSetupProps): SetupParts => {
  const { status, selectedBundleId } = props;
  const selected = bundleById(status, selectedBundleId);
  const recommended = status.bundles.find((bundle) => bundle.variant === RECOMMENDED_VARIANT) ?? null;
  return {
    heading: 'Choose a voice to get started',
    main: (
      <>
        <p>{INTRO}</p>
        <div className={styles.cards} role="radiogroup" aria-label="Model">
          {status.bundles.map((bundle) => <RadioCard key={bundle.id} bundle={bundle} checked={bundle.id === selectedBundleId} onSelect={() => props.onSelect(bundle.id)} />)}
        </div>
        <div className={styles.stack}>
          {recommended && <p>Not sure? Pick {recommended.displayName}.</p>}
          <p>All have the same {VOICE_COUNT} English voices. You can switch later in Settings.</p>
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

const PHASE_PARTS: Record<InstallPhase, (props: ModelSetupProps) => SetupParts> = {
  choosing: choosingParts,
  engineLoading: (props) => ({ heading: 'Starting the voice…', main: <SampleCard voiceLabel={props.voiceLabel} state="engineLoading" failed={false} onToggle={props.onToggleSample} /> }),
  engineFailed: (props) => ({
    heading: DOWNLOADING_HEADING,
    main: <NoticeBox kind="error"><p>The voice couldn't start. Try again, or download it again.</p></NoticeBox>,
    actions: <><button type="button" className={primary} onClick={props.onRetryEngine}>Try again</button><button type="button" className={desktop.textButton} onClick={props.onDownloadAgain}>Download again</button></>,
  }),
  ready: (props) => ({
    heading: 'The voice is ready',
    main: <SampleCard voiceLabel={props.voiceLabel} state={props.sample} failed={props.sampleFailed} onToggle={props.onToggleSample} />,
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

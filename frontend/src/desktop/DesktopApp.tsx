import { useCallback, useEffect, useRef, useState } from 'react';
import type { Route } from '../config/routes';
import { useAfterFileDialog } from '../hooks/useAfterFileDialog';
import { useHashRoute } from '../hooks/useHashRoute';
import { LibraryPage } from '../pages/LibraryPage/LibraryPage';
import { ReaderPage } from '../pages/ReaderPage/ReaderPage';
import type { ReaderShell } from '../pages/ReaderPage/readerShell';
import controls from '../styles/controls.module.css';
import type { DocumentId } from '../types/document';
import type { AppInfo, DesktopCommands, Licenses, LinkId, ModelStatus } from './commands/types';
import { FirstRunPage } from './components/ModelSetup/ModelSetup';
import { NoVoiceRow } from './components/NoVoiceRow/NoVoiceRow';
import { AboutTab } from './components/SettingsDialog/AboutTab';
import { SettingsButton } from './components/SettingsDialog/SettingsButton';
import { SettingsDialog } from './components/SettingsDialog/SettingsDialog';
import type { SettingsTab } from './components/SettingsDialog/SettingsDialog';
import { VoiceModelTab } from './components/SettingsDialog/VoiceModelTab';
import { VoiceStrip } from './components/VoiceStrip/VoiceStrip';
import { DesktopExportToast } from './DesktopExportToast';
import { DesktopReaderExport } from './DesktopReaderExport';
import { useExportAvailability } from './hooks/useExportAvailability';
import { useExportSaves } from './hooks/useExportSaves';
import { useExportToasts } from './hooks/useExportToasts';
import { useModelSetup } from './hooks/useModelSetup';
import { useSettingsShortcut } from './hooks/useSettingsShortcut';
import { useVoiceSample } from './hooks/useVoiceSample';
import { hasNoVoice, voiceStatusText } from './utils/voiceStatus';

/** UX1-A: the setup screen opens at start while there's no working voice, unless "Not now" was chosen with none installed. */
const needsSetup = (status: ModelStatus): boolean => status.engine !== 'ready' && status.engine !== 'loading' && !(status.setupDismissed && hasNoVoice(status));

/** The engine the reader was opened with: a reader opened before the voice was ready asks for health and voices again. */
const engineKey = (status: ModelStatus): string => `${status.engine}:${status.bundles.find((bundle) => bundle.active)?.id ?? ''}`;

/** The desktop app: the web app's library and reader, with the voice-model setup, settings, desktop export and toasts. */
export const DesktopApp = ({ commands }: { commands: DesktopCommands }) => {
  const { route, navigate } = useHashRoute();
  const setup = useModelSetup(commands);
  const { status } = setup;
  const sample = useVoiceSample(status?.engine === 'ready');
  const saves = useExportSaves(commands);
  const toasts = useExportToasts(commands);
  const exporter = useExportAvailability(commands);
  const [setupOpen, setSetupOpen] = useState<boolean | null>(null);
  const [settingsTab, setSettingsTab] = useState<SettingsTab | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [licenses, setLicenses] = useState<Licenses | null>(null);
  const [fileToImport, setFileToImport] = useState<File | null>(null);
  const [railShown, setRailShown] = useState(false);
  /** The book whose reader should open with the popover on its export section. */
  const [openAtExportFor, setOpenAtExportFor] = useState<DocumentId | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const afterFileDialog = useAfterFileDialog();

  // Decided once, on the first status; afterwards only the user opens and closes it.
  if (status !== null && setupOpen === null) setSetupOpen(needsSetup(status));

  useEffect(() => {
    commands.appInfo().then(setInfo).catch((cause: unknown) => console.error('App info failed', cause));
  }, [commands]);

  // The license texts are read the first time the settings open.
  const settingsOpen = settingsTab !== null;
  useEffect(() => {
    if (!settingsOpen || licenses !== null) return;
    commands.licenses().then(setLicenses).catch((cause: unknown) => console.error('Licenses failed', cause));
  }, [commands, settingsOpen, licenses]);

  // Exports continue after an engine reload, so the toast looks again once the engine has settled (the shell can't
  // list exports while it swaps the engine).
  const currentEngine = status === null ? null : engineKey(status);
  const settled = status !== null && status.engine !== 'loading';
  const { wake } = toasts;
  useEffect(() => {
    if (settled) wake();
  }, [currentEngine, settled, wake]);

  const openSettings = useCallback(() => setSettingsTab('model'), []);
  useSettingsShortcut(openSettings);
  const clearFileToImport = useCallback(() => setFileToImport(null), []);
  const clearOpenAtExport = useCallback(() => setOpenAtExportFor(null), []);

  if (status === null || currentEngine === null) return null;

  const activeBundle = status.bundles.find((bundle) => bundle.active) ?? null;
  const goTo = (target: Route) => {
    setSetupOpen(false);
    navigate(target);
  };
  const goToLibrary = () => goTo({ name: 'library' });
  const openSetup = () => setSetupOpen(true);
  const openLink = (link: LinkId) => {
    commands.openLink(link).catch((cause: unknown) => console.error('Open link failed', cause));
  };
  const toastJob = toasts.current;

  const readerShell: ReaderShell = {
    renderExportPanel: (context) => <DesktopReaderExport context={context} status={status} saves={saves} exporter={exporter} onExportActive={wake} />,
    noVoiceNotice: hasNoVoice(status) ? <NoVoiceRow onGetVoice={openSetup} /> : null,
    extraActions: <button type="button" className={controls.button} onClick={openSettings}>Settings…</button>,
    voiceStatus: voiceStatusText(status),
    onRailShownChange: setRailShown,
    openAtExport: route.name === 'reader' && route.documentId === openAtExportFor,
    onOpenedAtExport: clearOpenAtExport,
  };

  const page = () => {
    if (setupOpen) {
      return (
        <FirstRunPage
          status={status}
          selectedBundleId={setup.selectedBundleId}
          onSelect={setup.select}
          onDownload={setup.download}
          onPause={setup.pause}
          onCancel={setup.cancel}
          onImport={setup.importModel}
          importKind={setup.importKind}
          onNotNow={() => {
            setup.dismissSetup();
            goToLibrary();
          }}
          onGoToLibrary={goToLibrary}
          onOpenPdf={() => fileInputRef.current?.click()}
          onRetryEngine={() => activeBundle && setup.activate(activeBundle.id)}
          onDownloadAgain={() => activeBundle && setup.download(activeBundle.id)}
          onOpenLink={openLink}
          voiceLabel={sample.voiceLabel}
          sample={sample.state}
          onToggleSample={sample.toggle}
        />
      );
    }
    if (route.name === 'reader') {
      return <ReaderPage key={`${route.documentId}/${currentEngine}`} documentId={route.documentId} onBack={() => {
        clearOpenAtExport();
        navigate({ name: 'library' });
      }} shell={readerShell} />;
    }
    const transfer = status.transfer;
    return (
      <LibraryPage
        onOpenDocument={(documentId) => navigate({ name: 'reader', documentId })}
        headerActions={<SettingsButton onOpen={openSettings} />}
        notice={<VoiceStrip status={status} onGetVoice={openSetup} onResume={() => transfer.kind === 'paused' && setup.download(transfer.bundleId)} onDetails={openSetup} />}
        fileToImport={fileToImport}
        onFileTaken={clearFileToImport}
      />
    );
  };

  return (
    <>
      {page()}
      {/* The setup screen's "Open a PDF…" opens the chooser right from the click; once the dialog has closed, the library imports the file. */}
      <input
        ref={fileInputRef}
        type="file"
        accept="application/pdf,.pdf"
        hidden
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = '';
          if (!file) return;
          afterFileDialog(() => {
            setFileToImport(file);
            goToLibrary();
          });
        }}
      />
      {settingsTab !== null && (
        <SettingsDialog
          initialTab={settingsTab}
          onClose={() => setSettingsTab(null)}
          modelTab={(
            <VoiceModelTab
              status={status}
              exportingBookTitle={activeBundle?.usedByExports[0]?.title ?? null}
              refusedDelete={setup.refusedDelete}
              onDismissRefusedDelete={setup.dismissRefusedDelete}
              onDownload={setup.download}
              onPause={setup.pause}
              onCancel={setup.cancel}
              onActivate={setup.activate}
              onDelete={setup.deleteBundle}
              onImport={setup.importModel}
              importKind={setup.importKind}
            />
          )}
          aboutTab={info && <AboutTab info={info} licenses={licenses} onOpenLink={openLink} />}
        />
      )}
      {toastJob && (
        <DesktopExportToast
          key={toastJob.id}
          job={toastJob}
          saves={saves}
          railOpen={railShown}
          onDismiss={() => toasts.dismiss(toastJob.id)}
          onOpenBook={() => {
            toasts.dismiss(toastJob.id);
            setOpenAtExportFor(toastJob.documentId);
            goTo({ name: 'reader', documentId: toastJob.documentId });
          }}
        />
      )}
    </>
  );
};

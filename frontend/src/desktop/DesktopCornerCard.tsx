import { useState } from 'react';
import type { DocumentId } from '../types/document';
import type { ModelBundleId } from './commands/types';
import { CommandErrorToast } from './components/CommandErrorToast/CommandErrorToast';
import { PhoneVersionToast } from './components/PhoneVersionToast/PhoneVersionToast';
import { PhoneVoiceCard } from './components/PhoneVoiceCard/PhoneVoiceCard';
import { DesktopExportToast } from './DesktopExportToast';
import type { CommandErrors } from './hooks/useCommandErrors';
import type { ExportSaves } from './hooks/useExportSaves';
import type { ExportToasts } from './hooks/useExportToasts';
import type { PhoneHelper } from './hooks/usePhoneHelper';

interface DesktopCornerCardProps {
  commandErrors: CommandErrors;
  toasts: ExportToasts;
  saves: ExportSaves;
  phone: PhoneHelper;
  /** The reader's chapters rail is open (the toasts move above the pill). */
  railOpen: boolean;
  /** "Open book" on an export's toast: the reader, with the export section in view. */
  onOpenBook: (documentId: DocumentId) => void;
  onDownloadVoice: (bundleId: ModelBundleId) => void;
}

/** One card at a time in the corner: a refused command first, then the export toast, the version toast and the phone's voice. */
export const DesktopCornerCard = ({ commandErrors, toasts, saves, phone, railOpen, onOpenBook, onDownloadVoice }: DesktopCornerCardProps) => {
  /** The voice whose download was started from the phone's card: the card stays closed for it. */
  const [phoneVoiceStarted, setPhoneVoiceStarted] = useState<ModelBundleId | null>(null);

  const errorToast = commandErrors.current;
  if (errorToast) return <CommandErrorToast key={errorToast.id} failure={errorToast} railOpen={railOpen} onDismiss={() => commandErrors.dismiss(errorToast.id)} />;

  const toastJob = toasts.current;
  if (toastJob) {
    return (
      <DesktopExportToast
        key={toastJob.id}
        job={toastJob}
        saves={saves}
        railOpen={railOpen}
        onDismiss={() => toasts.dismiss(toastJob.id)}
        onOpenBook={() => {
          toasts.dismiss(toastJob.id);
          onOpenBook(toastJob.documentId);
        }}
      />
    );
  }

  const versionMismatch = phone.status?.versionMismatch ?? null;
  if (versionMismatch) return <PhoneVersionToast mismatch={versionMismatch} railOpen={railOpen} onDismiss={phone.actions.dismissVersionMismatch} />;

  const phoneVoiceRequest = phone.status?.modelRequest ?? null;
  if (phoneVoiceRequest === null || phoneVoiceRequest.bundleId === phoneVoiceStarted) return null;
  return (
    <PhoneVoiceCard
      request={phoneVoiceRequest}
      onDownload={() => {
        onDownloadVoice(phoneVoiceRequest.bundleId);
        setPhoneVoiceStarted(phoneVoiceRequest.bundleId);
      }}
      onNotNow={phone.actions.dismissModelRequest}
    />
  );
};

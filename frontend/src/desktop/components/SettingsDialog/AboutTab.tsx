import { useState } from 'react';
import controls from '../../../styles/controls.module.css';
import type { AppInfo, DesktopOs, Licenses, LinkId } from '../../commands/types';
import { LINK_LABEL, SETTINGS_SHORTCUT_KEY } from '../../config';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import desktop from '../../styles/controls.module.css';
import { versionLine } from '../../utils/format';
import { ExternalLinkIcon } from '../icons';
import { NoticeBox } from '../NoticeBox/NoticeBox';
import { licensesFor } from './licenses';
import styles from './SettingsDialog.module.css';

/** `licenses` is null until the shell has answered. */
interface AboutTabProps { info: AppInfo; licenses: Licenses | null; onOpenLink: (linkId: LinkId) => void; }

const FFMPEG_CREDIT: Record<DesktopOs, string> = {
  linux: "Audiobook files: your system's FFmpeg.",
  windows: 'Audiobook files: FFmpeg (LGPL-2.1), included. Its source code and our build script are attached to each release.',
};
const LINKS: readonly (keyof typeof LINK_LABEL)[] = ['releases', 'source'];

const SHORTCUTS: readonly { keys: readonly string[]; does: string }[] = [
  { keys: ['Space'], does: 'Play or pause' },
  { keys: ['←', '→'], does: 'Previous or next sentence' },
  { keys: ['C'], does: 'Chapters' },
  { keys: [`Ctrl+${SETTINGS_SHORTCUT_KEY}`], does: 'Settings' },
  { keys: ['Esc'], does: 'Close a panel or dialog' },
];

const noop = () => {};

const LinkButton = ({ children, onClick }: { children: React.ReactNode; onClick: () => void }) => (
  <button type="button" className={desktop.textButton} onClick={onClick}>{children} <ExternalLinkIcon /></button>
);

/** UX4-A: the About tab of the settings dialog. */
export const AboutTab = ({ info, licenses, onOpenLink }: AboutTabProps) => {
  const [openLicense, setOpenLicense] = useState<number | null>(null);
  // The version line is already on screen, so a refused clipboard needs no fallback.
  const { copied, copy } = useCopyFeedback(noop);
  const rows = licensesFor(info.os);
  // Null in a build made without the generated list: nothing to open.
  const otherPackages = licenses?.otherPackages ?? null;

  return (
    <div className={styles.about}>
      <div>
        <div className={styles.appName}>Lectrice</div>
        <div className={desktop.muted}>Reads PDFs aloud, locally</div>
      </div>
      <div className={desktop.actions}>
        <span>{versionLine(info)}</span>
        <button type="button" className={controls.button} onClick={() => copy(versionLine(info))}>{copied ? 'Copied' : 'Copy details'}</button>
      </div>
      <div className={styles.links}>
        {LINKS.map((link) => <LinkButton key={link} onClick={() => onOpenLink(link)}>{LINK_LABEL[link]}</LinkButton>)}
      </div>
      <div>
        <h3 className={styles.keysHeading}>Keyboard</h3>
        <dl className={styles.keys}>
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.does} className={styles.keyRow}>
              <dt className={styles.keyNames}>{shortcut.keys.map((key) => <kbd key={key}>{key}</kbd>)}</dt>
              <dd>{shortcut.does}</dd>
            </div>
          ))}
        </dl>
      </div>
      <div className={styles.credits}>
        <p>Voice: Kokoro 82M by hexgrad (Apache-2.0), run with sherpa-onnx by k2-fsa (Apache-2.0).</p>
        <p>PDF text: PDFium (BSD-3-Clause).</p>
        <p>{FFMPEG_CREDIT[info.os]}</p>
      </div>
      <NoticeBox kind="info">
        <p>Lectrice includes espeak-ng, licensed under the GNU GPL version 3. The complete source code is on GitHub (kia-salvati/lectrice-desktop and kia-salvati/lectrice) at the tag of this version.</p>
      </NoticeBox>
      <div className={styles.licenses}>
        {rows.map((row, index) => {
          const open = openLicense === index;
          const text = licenses?.texts[row.textId];
          return (
            <div key={row.component}>
              <div className={styles.licenseRow}>
                <span>{row.component}</span>
                <span className={styles.licenseName}>{row.license}</span>
                {text !== undefined && <button type="button" className={desktop.textButton} aria-expanded={open} onClick={() => setOpenLicense(open ? null : index)}>{open ? 'Hide text' : 'Show text'}</button>}
              </div>
              {open && text !== undefined && <pre className={styles.licenseText} tabIndex={0}>{text}</pre>}
            </div>
          );
        })}
        {otherPackages !== null && (
          <div className={styles.licenseRow}>
            <span>All other Rust and npm packages ({otherPackages})</span>
            <span className={styles.licenseName}>various</span>
            <LinkButton onClick={() => onOpenLink('thirdPartyLicenses')}>Open full list</LinkButton>
          </div>
        )}
      </div>
    </div>
  );
};

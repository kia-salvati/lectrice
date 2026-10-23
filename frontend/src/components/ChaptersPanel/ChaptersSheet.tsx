import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import controls from '../../styles/controls.module.css';
import type { TocEntry } from '../../types/document';
import { childrenText, flattenTocTree, nodeView } from '../../utils/toc';
import type { TocLocation, TocNode } from '../../utils/toc';
import { ChevronIcon, CloseIcon } from '../icons/icons';
import { ChaptersEmpty } from './ChaptersEmpty';
import panel from './ChaptersPanel.module.css';
import styles from './ChaptersSheet.module.css';
import { centreIn, findDeepestRendered, findTitleButton, titleButtons, titleOfRow } from './scrollInto';
import { TocRow } from './TocRow';

interface ChaptersSheetProps {
  /** For aria-controls on the pill's chapters button. */
  id: string;
  bookTitle: string;
  toc: readonly TocEntry[];
  tree: readonly TocNode[];
  sentenceCount: number;
  currentSentenceId: number | null;
  location: TocLocation | null;
  pageLabelOf: (pageIndex: number) => string;
  /** Phones: a modal over an inert page. Elsewhere a disclosure the pill stays usable under. */
  isModal: boolean;
  onSelect: (sentenceId: number) => void;
  onClose: () => void;
}

const NO_PATH: readonly number[] = [];

/** Which level is shown (the children of `parentIndex`, or the top level) and which row should take focus. */
interface SheetView { parentIndex: number | null; focusIndex: number | null; }

/** Opens on the current chapter's sections, or on its siblings when it has none. */
const initialView = (nodeAt: ReadonlyMap<number, TocNode>, location: TocLocation | null): SheetView => {
  const chapter = location === null ? null : nodeAt.get(location.chapterIndex);
  if (!chapter) return { parentIndex: null, focusIndex: null };
  return { parentIndex: chapter.children.length > 0 ? chapter.index : chapter.parentIndex, focusIndex: null };
};

/** One level at a time, lifted out of the pill: tap "›" to drill into a chapter, "‹" to come back, a title to jump. */
export const ChaptersSheet = ({ id, bookTitle, toc, tree, sentenceCount, currentSentenceId, location, pageLabelOf, isModal, onSelect, onClose }: ChaptersSheetProps) => {
  const nodeAt = useMemo(() => new Map(flattenTocTree(tree).map((node) => [node.index, node])), [tree]);
  const [view, setView] = useState<SheetView>(() => initialView(nodeAt, location));
  const dialogRef = useRef<HTMLDialogElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const parent = view.parentIndex === null ? null : nodeAt.get(view.parentIndex) ?? null;
  const shown = parent ? parent.children : tree;
  const grandparent = parent !== null && parent.parentIndex !== null ? nodeAt.get(parent.parentIndex) ?? null : null;
  const titleId = `${id}-title`;
  const path = location?.path ?? NO_PATH;
  const pathRef = useRef(path);
  useLayoutEffect(() => {
    pathRef.current = path;
  });

  // Before the focus effect below: opening a dialog focuses its first control, which the current row then replaces.
  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (isModal) dialog.showModal();
    else dialog.show();
    return () => dialog.close();
  }, [isModal]);

  // Outside the modal case the sheet is a disclosure: a click elsewhere closes it (the pill's own button toggles it itself).
  useEffect(() => {
    if (isModal) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Element)) return;
      if (dialogRef.current?.contains(event.target) || event.target.closest('[data-chapters-trigger]')) return;
      onClose();
    };
    window.addEventListener('pointerdown', handlePointerDown);
    return () => window.removeEventListener('pointerdown', handlePointerDown);
  }, [isModal, onClose]);

  // On open and after each drill or back: centre and focus the row that matters at this level (the path of that moment).
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const row = (view.focusIndex === null ? null : findTitleButton(body, view.focusIndex)) ?? findDeepestRendered(body, pathRef.current) ?? titleButtons(body)[0] ?? null;
    if (row) {
      centreIn(body, row);
      row.focus();
    } else {
      closeRef.current?.focus();
    }
  }, [view]);

  const drillInto = (node: TocNode) => setView({ parentIndex: node.index, focusIndex: null });
  const goBack = () => {
    if (parent === null) return;
    setView({ parentIndex: parent.parentIndex, focusIndex: parent.index });
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey || !bodyRef.current) return;
    if (event.key === 'Escape' || event.key === 'c' || event.key === 'C') {
      event.preventDefault();
      onClose();
      return;
    }
    const title = titleOfRow(event.target);
    if (!title) return;
    const titles = titleButtons(bodyRef.current);
    const at = titles.indexOf(title);
    const node = nodeAt.get(Number(title.dataset.tocIndex));
    if (!node) return;
    const focusAt = (k: number) => titles[k]?.focus();
    switch (event.key) {
      case 'ArrowDown': focusAt(at + 1); break;
      case 'ArrowUp': focusAt(at - 1); break;
      case 'Home': focusAt(0); break;
      case 'End': focusAt(titles.length - 1); break;
      case 'ArrowRight': if (node.children.length > 0) drillInto(node); break;
      case 'ArrowLeft': goBack(); break;
      default: return;
    }
    event.preventDefault();
  };

  return (
    // tabIndex: a click on the sheet's own background keeps focus (and so the key trap) inside it.
    <dialog ref={dialogRef} id={id} className={styles.sheet} aria-labelledby={titleId} tabIndex={-1} data-own-keys data-own-scroll onKeyDown={handleKeyDown} onCancel={(event) => { event.preventDefault(); onClose(); }}>
      <header className={styles.head}>
        {parent && <button type="button" className={styles.back} onClick={goBack}>‹ {grandparent ? grandparent.entry.title : 'All chapters'}</button>}
        <h2 id={titleId} className={styles.title}>{parent ? parent.entry.title : bookTitle}</h2>
        <button type="button" ref={closeRef} className={controls.iconButton} aria-label="Close chapters" onClick={onClose}><CloseIcon /></button>
      </header>
      <div ref={bodyRef} className={styles.body}>
        {tree.length === 0 ? <ChaptersEmpty /> : (
          <nav aria-label="Chapters">
            <ul className={panel.list}>
              {shown.map((node) => {
                const sheetView = nodeView(toc, node, location, currentSentenceId, sentenceCount);
                const trailing = node.children.length > 0
                  ? <button type="button" className={styles.drill} aria-label={`Show ${childrenText(node)} of ${node.entry.title}`} onClick={() => drillInto(node)}><span className={styles.drillSize}>{childrenText(node)}</span><ChevronIcon /></button>
                  : undefined;
                return (
                  <li key={node.index}>
                    <TocRow
                      node={node}
                      layout="drill"
                      pageLabel={pageLabelOf(node.entry.pageIndex)}
                      progress={sheetView.progress}
                      isHere={sheetView.isHere}
                      isPart={false}
                      percent={sheetView.percent}
                      bar={sheetView.fillPercent === null ? null : { fillPercent: sheetView.fillPercent, widthPercent: null }}
                      sizeText={null}
                      trailing={trailing}
                      onSelect={onSelect}
                    />
                  </li>
                );
              })}
            </ul>
          </nav>
        )}
      </div>
    </dialog>
  );
};

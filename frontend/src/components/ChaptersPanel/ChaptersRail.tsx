import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import controls from '../../styles/controls.module.css';
import type { TocEntry } from '../../types/document';
import { childrenText, entryLength, filterTocTree, flattenTocTree, isChapterNode, isPartNode, nodeView } from '../../utils/toc';
import type { TocLocation, TocNode } from '../../utils/toc';
import { ChevronIcon, CloseIcon, ListIcon, LocateIcon } from '../icons/icons';
import { ChaptersEmpty } from './ChaptersEmpty';
import styles from './ChaptersRail.module.css';
import panel from './ChaptersPanel.module.css';
import { centreIn, findDeepestRendered, keepInView, titleButtons, titleOfRow } from './scrollInto';
import { TocRow } from './TocRow';

interface ChaptersRailProps {
  bookTitle: string;
  toc: readonly TocEntry[];
  tree: readonly TocNode[];
  /** One past the last sentence id: where the last part ends. */
  sentenceCount: number;
  currentSentenceId: number | null;
  location: TocLocation | null;
  pageLabelOf: (pageIndex: number) => string;
  /** Each change scrolls the rail to the current chapter and focuses its row. */
  focusRequest: number;
  onSelect: (sentenceId: number) => void;
  onClose: () => void;
  /** Escape: the reader wants focus back where it was. */
  onLeave: () => void;
}

/** Bars never get thinner than this share of the widest chapter, so short chapters stay visible. */
const MIN_BAR_WIDTH_PERCENT = 12;

/** Before the first entry there is no current row; a shared constant keeps the effects' dependency stable. */
const NO_PATH: readonly number[] = [];

type Settle = 'centre' | 'focus' | null;

const canExpand = (node: TocNode): boolean => node.children.length > 0 && !isPartNode(node);

/**
 * The whole book as one tree: parts as group headings, a length- and progress-bar on every chapter, sections inline.
 * View state: which rows are expanded (the current path at first) and the filter.
 */
export const ChaptersRail = ({ bookTitle, toc, tree, sentenceCount, currentSentenceId, location, pageLabelOf, focusRequest, onSelect, onClose, onLeave }: ChaptersRailProps) => {
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set(location?.path ?? []));
  const [filter, setFilter] = useState('');
  const [settle, setSettle] = useState<Settle>('centre');
  const railRef = useRef<HTMLElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const filterRef = useRef<HTMLInputElement>(null);

  const nodes = useMemo(() => flattenTocTree(tree), [tree]);
  const nodeAt = useMemo(() => new Map(nodes.map((node) => [node.index, node])), [nodes]);
  const isFlat = useMemo(() => nodes.every((node) => node.children.length === 0), [nodes]);
  const widestChapter = useMemo(() => Math.max(1, ...nodes.filter(isChapterNode).map((node) => entryLength(toc, node.index, sentenceCount))), [nodes, toc, sentenceCount]);
  const isFiltering = filter.trim() !== '';
  const shown = useMemo(() => filterTocTree(tree, filter), [tree, filter]);
  const path = location?.path ?? NO_PATH;
  const pathRef = useRef(path);
  useEffect(() => {
    pathRef.current = path;
  });

  /** Expands the current path, centres the current row and moves focus into it. */
  const goToCurrent = (currentPath: readonly number[]) => {
    setFilter('');
    setExpanded((previous) => new Set([...previous, ...currentPath]));
    setSettle('focus');
  };

  // Opened by the C key or the popover's "Chapters…".
  useEffect(() => {
    if (focusRequest > 0) goToCurrent(pathRef.current);
  }, [focusRequest]);

  useLayoutEffect(() => {
    if (settle === null || !bodyRef.current) return;
    const row = findDeepestRendered(bodyRef.current, path);
    if (row) {
      centreIn(bodyRef.current, row);
      if (settle === 'focus') row.focus();
    }
    setSettle(null);
  }, [settle, path]);

  // As playback moves on, keep the current row in view unless the reader is working inside the rail.
  useEffect(() => {
    if (!bodyRef.current || railRef.current?.contains(document.activeElement)) return;
    const row = findDeepestRendered(bodyRef.current, path);
    if (row) keepInView(bodyRef.current, row);
  }, [path]);

  const toggleExpanded = (index: number, open: boolean) => setExpanded((previous) => {
    const next = new Set(previous);
    if (open) next.add(index);
    else next.delete(index);
    return next;
  });

  const toggleAll = () => setExpanded(expanded.size > 0 ? new Set() : new Set(nodes.filter(canExpand).map((node) => node.index)));

  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey || !bodyRef.current) return;
    if (event.target === filterRef.current) {
      if (event.key === 'Escape') {
        event.preventDefault();
        if (filter) setFilter('');
        else onLeave();
      } else if (event.key === 'ArrowDown') {
        event.preventDefault();
        titleButtons(bodyRef.current)[0]?.focus();
      }
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      onLeave();
      return;
    }
    if (event.key === 'c' || event.key === 'C') {
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
    const isOpen = isFiltering || isPartNode(node) || expanded.has(node.index);
    const focusAt = (k: number) => titles[k]?.focus();
    switch (event.key) {
      case 'ArrowDown': focusAt(at + 1); break;
      case 'ArrowUp': focusAt(at - 1); break;
      case 'Home': focusAt(0); break;
      case 'End': focusAt(titles.length - 1); break;
      case 'ArrowRight':
        if (node.children.length === 0) break;
        if (canExpand(node) && !isOpen) toggleExpanded(node.index, true);
        else focusAt(at + 1);
        break;
      case 'ArrowLeft':
        if (canExpand(node) && isOpen && !isFiltering) toggleExpanded(node.index, false);
        else if (node.parentIndex !== null) titles.find((button) => Number(button.dataset.tocIndex) === node.parentIndex)?.focus();
        break;
      default: return;
    }
    event.preventDefault();
  };

  const renderNodes = (list: readonly TocNode[], id?: string): React.ReactNode => (
    <ul className={panel.list} id={id}>
      {list.map((node) => {
        const view = nodeView(toc, node, location, currentSentenceId, sentenceCount);
        const expandable = canExpand(node);
        const isOpen = isFiltering || view.isPart || expanded.has(node.index);
        const childrenId = `rail-children-${node.index}`;
        const leading = isFlat || view.isPart ? undefined : (expandable
          ? <button type="button" className={panel.disclosure} aria-expanded={isOpen} aria-controls={isOpen ? childrenId : undefined} aria-label={`${isOpen ? 'Hide' : 'Show'} ${childrenText(node)} of ${node.entry.title}`} onClick={() => toggleExpanded(node.index, !isOpen)}><ChevronIcon /></button>
          : <span />);
        const bar = view.fillPercent === null ? null : { fillPercent: view.fillPercent, widthPercent: Math.max(MIN_BAR_WIDTH_PERCENT, Math.round((entryLength(toc, node.index, sentenceCount) / widestChapter) * 100)) };
        return (
          <li key={node.index}>
            <TocRow
              node={node}
              layout={isFlat || view.isPart ? 'plain' : 'tree'}
              pageLabel={pageLabelOf(node.entry.pageIndex)}
              progress={view.progress}
              isHere={view.isHere}
              isPart={view.isPart}
              percent={view.percent}
              bar={bar}
              sizeText={expandable && !isOpen ? childrenText(node) : null}
              leading={leading}
              onSelect={onSelect}
            />
            {node.children.length > 0 && isOpen && renderNodes(node.children, childrenId)}
          </li>
        );
      })}
    </ul>
  );

  return (
    // tabIndex: a click on the rail's own background keeps focus (and so the key trap) inside it.
    <nav ref={railRef} className={styles.rail} aria-label="Chapters" tabIndex={-1} data-own-keys data-own-scroll onKeyDown={handleKeyDown}>
      <div className={styles.head}>
        <h2 className={styles.bookTitle}>{bookTitle}</h2>
        <button type="button" className={`${controls.iconButton} ${styles.headButton}`} aria-label="Scroll the rail to the current chapter" title="Current chapter" onClick={() => goToCurrent(path)}><LocateIcon /></button>
        <button type="button" className={`${controls.iconButton} ${styles.headButton}`} aria-label="Close the chapters rail" title="Close (C)" onClick={onClose}><CloseIcon /></button>
      </div>
      <div className={styles.tools}>
        <input ref={filterRef} type="search" className={styles.filter} placeholder="Filter titles" aria-label="Filter chapters by title" value={filter} onChange={(event) => setFilter(event.target.value)} />
        {!isFlat && <button type="button" className={`${controls.button} ${styles.toggleAll}`} onClick={toggleAll}>{expanded.size > 0 ? 'Collapse all' : 'Expand all'}</button>}
      </div>
      <div ref={bodyRef} className={styles.body}>
        {tree.length === 0 && <ChaptersEmpty />}
        {tree.length > 0 && shown.length === 0 && <p className={panel.noMatch}>No titles match.</p>}
        {shown.length > 0 && renderNodes(shown)}
      </div>
    </nav>
  );
};

/** The way back once the rail is closed: a small tab at the top-left edge. */
export const ChaptersRailHandle = ({ onOpen }: { onOpen: () => void }) => (
  <button type="button" className={styles.handle} aria-label="Show the chapters rail" onClick={onOpen}><ListIcon />Chapters</button>
);

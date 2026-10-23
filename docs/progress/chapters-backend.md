# Chapters: backend `toc` (done 2026-10-23)

Branch `feature/chapters`, nothing committed. The design brief for Fable is `docs/2026-10-23-chapters-brief.md`.

## What changed
- **`Document.toc: TocEntry[]`** (`lectrice-domain/src/document.rs`): `{ id, title, level (1–3), sentenceId, pageIndex }`,
  flat, in reading order. `#[serde(default)]`, so documents saved before load with `toc: []` until re-cleaned.
- **`lectrice-domain/src/cleaning/toc.rs`** (new, pure): `build_toc(outline, blocks)`.
  1. Outline titles are cleaned: the part before a zero-width space (LinkedIn joins a post title and its first sentence with
     one), invisible chars dropped, whitespace collapsed, cut at a word after 120 chars with "…".
  2. Junk is skipped: empty titles, front matter (`cover`, `title page`, `copyright (page)`, `(table of) contents`,
     `Tweet This Book!`), entries whose page has no text (dropped TOC/index pages), and unmatched entries on the same page as
     the entry before (LinkedIn's sentence bookmarks).
  3. Each entry is matched to a heading block: same core words (numbering words like `Chapter 5` / `PART I` ignored), or one
     a word-prefix of the other, within ±1 page, searching forward only. It points at that heading's first sentence. An
     unmatched entry points at the first sentence of its page. When a matched heading is longer (GoF splits one title over two
     bookmarks), the heading's full title is used. Entries must strictly advance.
  4. If fewer than half of the kept entries matched a heading (or there's no outline), the **spoken heading blocks** are used
     instead (block level, same junk filter).
  5. Levels: shifted so the top is 1, never more than one deeper than the entry before, deeper than 3 dropped.
- `CleanedDocument` carries the PDF `outline` through to `build_document` (app), which calls `build_toc` once sentence ids exist.
  `headings.rs`: `core_words` and `PAGE_SLACK` are now `pub(super)` (shared with `toc.rs`).
- **`CLEANER_VERSION` 2 → 3**, so saved books are re-cleaned on open and gain `toc`.
- Spec §5 (example + a `toc` paragraph) and §7 (`GET /api/documents/{id}` includes `toc`) updated.
- Frontend: `types/document.ts` gains `TocLevel`, `TocEntry`, `CleanDocument.toc`. Mock: every mock document gets a `toc`
  built from its headings (like the server's fallback); "The Long Book" is now 15 chapters × 3–8 sections × 3 sub-sections
  (304 entries, 7,018 sentences, long titles, "Continue" lands in 7.3); new "Meeting Notes (no chapters)" with `toc: []`.
  All mock text is invented. `repeatChapter` was replaced by `buildChapteredBody`.

## The five books (entry counts; titles only)
| Book | Outline entries | `toc` entries | Source | First chapters | Without the outline (headings) |
|---|---|---|---|---|---|
| Clean Architecture | 226 | 222 (levels 1–3: parts › chapters › sections) | outline | `PART I Introduction`, `Chapter 1 What Is Design and Architecture?`, `Chapter 2 A Tale of Two Values` | 240 |
| Designing Distributed Systems | 139 | 83 (level-4 bookmarks dropped) | outline | `Chapter 1. Introduction`, `Part I. Single-Node Patterns`, `Chapter 2. The Sidecar Pattern` | 89 |
| Design Patterns (GoF) | 392 | 390 | outline | `1. Introduction`, `2. A Case Study: Design a Document Editor`, `3. Creational Patterns` | 480 (was 398 before the review's level-rule fix; see chapters-backend-review.md) |
| LinkedIn Posts 2024 | 215 | 198 (flat, all level 1) | outline | `Big Endian vs Little Endian`, `How do we incorporate Event Sourcing into the systems?`, `How can Cache Systems go wrong` | 202 |
| The Kubernetes Book | 127 | 125 (chapters › sections) | outline | `0: Preface`, `1: Kubernetes primer`, `2: Kubernetes principles of operation` | 330 |

Dropped from the outlines: CA `Title Page`, `Copyright Page`, `Contents`, `Index`; DDS `Copyright`, `Table of Contents`,
`Index` (+ 53 level-4 entries); GoF `Contents` and the second half of a split title; K8s `Table of Contents`, `Index`;
LinkedIn the empty root, 2 empty entries, 4 sentence bookmarks and 9 bookmarks named `1.`–`9.` (its children were promoted to
level 1). Front matter that is real reading (Foreword, Preface, Dedication, About the Author) is kept. All sentence ids increase
and every level step is ≤ 1 (checked by the test).

## Tests
- `toc.rs` unit tests (11): outline → heading sentences; unmatched entry → its page; shorter/longer title match; front-matter,
  empty and textless entries skipped; same-page extras skipped; zero-width-space cut and long-title cut; levels start at 1,
  never skip, stop at 3; heading fallback without an outline (silent headings skipped); fallback when the outline mostly
  misses; split title completed from the heading; empty toc.
- `document.rs`: `toc` JSON shape (camelCase) and a v2 document without `toc` still loads. `build.rs`: toc built with sentence
  ids. API route test: `GET /api/documents/{id}` returns `toc`. Repository round-trip test now includes a toc entry.
- **Per-book regression** `lectrice-api/tests/toc_books.rs` (`#[ignore]`, reads `~/Desktop/*.pdf` directly, **skips a book that
  isn't there**, stores nothing): entry count, first 4 titles + levels, one later entry, well-formedness, and the heading
  fallback (> half as many entries, well-formed). Run:
  `cargo test --release -p lectrice-api --test toc_books -- --ignored --nocapture` (2.7 s, prints the first 30 entries locally).
- `cargo build`, `cargo test` (309 pass), the ignored toc test, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`:
  all pass. Frontend: `npm run build`, `npm run lint` (0 warnings), `npm test` (13/13).

## Notes
- Raising the cleaner version also changes the audio cache key (`c3/`), so audio cached under version 2 is regenerated as you
  listen; old `c2/` folders are evicted by the normal cache cap.
- Only the outline's titles are used when it's trusted; heading-only levels (e.g. K8s's level-3 headings) are not added to it.
- The heading fallback (not used by any of the 5 books) includes title pages as small headings (e.g. a cover's author name);
  acceptable for PDFs without bookmarks.

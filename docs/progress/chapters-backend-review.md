# Chapters backend: review (2026-10-23)

Reviewed the backend `toc` work on `feature/chapters` (`git diff -- backend`, `cleaning/toc.rs`, `lectrice-api/tests/toc_books.rs`)
against the spec §5/§7, `rules/BACKEND_RULES.md` and CLAUDE.md. The JSON contract (`toc: [{ id, title, level, sentenceId, pageIndex }]`)
is unchanged.

## Findings → fixes
| # | Finding | Verified by | Fix + regression test |
|---|---|---|---|
| 1 | **Level normalization made siblings into a staircase.** `(level - top + 1).min(previous + 1)` caps each entry one below the one before, so an outline that starts deeper than its top level (`[A L2, B L2, C L1]`) gave `A 1, B 2` (B shown as A's child). The same bug made level-4 headings right under level-2 ones alternate between depth 3 and dropped. | Probe test: `[("…background", 1), ("Orchestration", 2), …]` | `normalize_levels` now uses a stack of open source levels: depth = 1 + open entries with a smaller level. Tests `entries_of_the_same_level_stay_siblings_when_the_outline_starts_deeper`, `a_shallower_entry_after_a_deep_one_closes_only_the_deeper_levels`; the existing level test still passes. |
| 2 | **Quadratic page lookup.** `page_start` walked every sentence from the start of the book for each outline entry without a heading. | 3,000 entries (half unmatched) × 90,000 sentences: 2.3 s (debug) | Sentences are flattened once (id order); the search starts right after the entry before (`partition_point`). Same case: 0.1 s. Test `unmatched_entries_of_a_long_book_are_found_quickly` (1 s budget). |
| 3 | **The ignored book test loaded PDFium before checking for books**, so `cargo test -- --ignored` on a fresh clone (no `vendor/pdfium/lib`, no books) would panic instead of skipping. | Read | `toc_books.rs` reads the books first and returns before loading PDFium when none is there. Checked with `HOME` pointed at an empty folder: all 5 print "skipped", test passes. |
| 4 | No test that a version-2 document (no `toc`) gains one when re-cleaned. | Read | `get_document::a_document_saved_before_the_toc_existed_gains_one_when_re_cleaned` (also checks the re-saved file has it). |
| 5 | `MAX_TOC_LEVEL` was `pub` but only used inside `toc.rs`. | grep | Made private. |
| 6 | The pipeline log didn't say how many toc entries were built. | Read | `toc_entries` count added to the existing "sentences split" log (a count, no text). |

## Checked, no change needed
- Outline → sentence mapping: forward-only heading search within ±1 page; entries must strictly advance. Added tests for **repeated
  titles** (`Summary` in two chapters maps to each one's own heading) and an **outline entry past the last page** (skipped).
- Empty outline / all-junk outline / mostly unmatched outline → heading fallback; no outline and no headings → `[]` (tested).
- Pages with no text → entry skipped (tested); one-level books (LinkedIn) stay flat at level 1.
- Cleaner version 3: `is_outdated` (`stored < current`) re-cleans from the saved PDF on open; old docs deserialize with `toc: []`.
- API: `GET /api/documents/{id}` returns `toc` in camelCase (route test); domain stays pure, `build_toc` is called from the app layer.
- No book text is logged; tracked test data is invented or titles only.
- On the 5 books, the outline-based toc is **identical before and after the fixes** (same entries and levels). Only the heading
  fallback (used by none of them) changes: GoF 398 → 480 entries (its level-4 headings under level-2 ones are now kept at depth 3
  instead of alternately dropped), Clean Architecture's first title-page headings are now siblings. The table in
  `chapters-backend.md` still shows the old fallback counts.

## Left as is (notes)
- When a matched heading is longer than the outline title, the heading's text is used (for GoF's split titles). If a PDF's heading
  block swallowed a sub-title line, the toc shows it too (K8s `Editions …`). Acceptable; revisit if a book shows body text there.
- An outline entry with no page matches the next heading with the same words anywhere ahead; rare (all 5 books have pages).
- `find_heading` scans the remaining headings per entry (O(entries × headings)); 0.1 s for 3,000 × 3,000 in debug, so not worth an
  early break that would assume heading pages are monotone.
- An entry placed by its page may land on a code/table block's first sentence (not spoken by default); the player decides what to
  play from there.

## Results
`cargo build`, `cargo test` (340 passed, 12 ignored), `cargo test -- --ignored` (12 passed, books on this machine),
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`: all pass.

Files changed in this review: `backend/crates/lectrice-domain/src/cleaning/toc.rs`, `backend/crates/lectrice-api/tests/toc_books.rs`,
`backend/crates/lectrice-app/src/documents/get_document.rs`, `backend/crates/lectrice-app/src/documents/pipeline.rs`,
`docs/2026-10-21-design-spec.md` (§5 level rule).

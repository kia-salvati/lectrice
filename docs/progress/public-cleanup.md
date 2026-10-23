# Public cleanup: no book text in git (2026-10-23)

Goal: the repo can be public. Nothing from the user's books (copyrighted) or the user's LinkedIn posts is in
tracked files. Titles, chapter and section names, and generic patterns ("Click here to view code image",
"Figure 2-1.", "Tweet This Book!") stay. Done in the `release/v1` worktree. Nothing committed.

## Remove from tracking (`git rm --cached`, the local copies stay)
The files are already gitignored.
- `docs/progress/B2-samples/clean-architecture-ch5-oo-programming.txt`
- `docs/progress/B2-samples/design-patterns-abstract-factory.txt`
- `docs/progress/B2-samples/designing-distributed-systems-ch2-sidecar.txt`
- `docs/progress/B2-samples/kubernetes-book-ch14-api-security-rbac.txt`
- `docs/progress/B2-samples/linkedin-posts-p8-30.txt`

Moved, so they show as deleted (now local only in `backend/test-data/books/`):
- `backend/crates/lectrice-domain/tests/fixtures/clean-architecture.jsonl`
- `backend/crates/lectrice-domain/tests/fixtures/design-patterns.jsonl`
- `backend/crates/lectrice-domain/tests/fixtures/designing-distributed-systems.jsonl`
- `backend/crates/lectrice-domain/tests/fixtures/kubernetes-book.jsonl`
- `backend/crates/lectrice-domain/tests/fixtures/linkedin-posts.jsonl`

## Added
- `backend/crates/lectrice-domain/tests/cleaning_invented.rs`: 25 tests on invented pages that copy the
  books' layouts (page size, fonts, sizes, positions, line pitch) with made-up words. They cover headers,
  footers and labels; TOC, index, copyright and bibliography pages; real vs syllable hyphens; outline headings
  (including ALL CAPS); bullets, symbol bullets, emoji bullets, dash-numbered items; footnote markers and
  footnotes; code with rebuilt indent; the code-image link; captions vs "Figure N shows" sentences; tables;
  URLs, citations, page references, short bare domains; styled letters; the step T inline-code and wrapped
  bold-word cases. They always run.
- This file.

## Changed
- `.gitignore`: ignores `/backend/test-data/` and `/docs/progress/B2-samples/`. `/models` and
  `/backend/vendor/pdfium/lib` lost their trailing slash so symlinks to them are ignored too.
- `backend/README.md`: where the local book data lives, how to rebuild the fixtures
  (`LECTRICE_REGEN_FIXTURES=1 …`) and the review samples (`lectrice-api clean … --pages …`, with the page ranges).
- `backend/crates/lectrice-domain/tests/cleaning_support/mod.rs`: reads `backend/test-data/books/`; returns
  `None` and prints one "book tests skipped: … missing; rebuild it with …" line to the real stderr when a
  fixture is missing.
- `backend/crates/lectrice-domain/tests/cleaning_{clean_architecture,design_patterns,distributed_systems,kubernetes,linkedin_posts,step_t_regressions}.rs`:
  return early when the fixture is missing; needles cut to a few words (or titles), full-sentence asserts
  replaced by short endings or shape checks (same blocks and rules checked).
- `backend/crates/lectrice-infra/tests/pdf_fixtures.rs`, `pdf_support/mod.rs`: write and check the fixtures in
  `backend/test-data/books/`.
- Unit tests and comments in `backend/crates/lectrice-domain/src/cleaning/`: `assemble.rs`, `block_text.rs`,
  `code.rs`, `footnotes.rs`, `hyphenation.rs`, `line_kinds.rs`, `mod.rs`, `normalize.rs`, `page_roles.rs`,
  `speech_filters.rs`, `tables.rs`: book sentences, code lines, ISBNs and the author's e-mail and handles
  replaced by invented text of the same shape.
- `research/2026-10-21-pdf-survey.md`: quoted prose, footnotes, post text and code listings replaced by
  descriptions (`'(body line, 78 chars)'`) or invented code. Counts, positions, sizes, page numbers,
  titles and findings unchanged.
- `docs/progress/T.md`, `docs/progress/B2.md`, `docs/2026-10-21-design-spec.md`: a few quoted sentences
  paraphrased.

## Checked, no change needed
- `frontend/src/api/mock/*` and `design/2026-10-06-reader-options.html`: invented text ("Designing Calm
  Software", a sleep-science sample). `design/screens/*.png` show the mock text.

## Verification (worktree)
- `cargo build`; `cargo test` without `backend/test-data/`: 322 passed, 0 failed, 11 ignored (book tests
  return early with the skip message). With it: 322 passed, 0 failed, 11 ignored.
  `cargo test -p lectrice-infra --test pdf_fixtures -- --ignored`: the local fixtures still match the books.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`: clean.
- `npm run build`, `npm run lint`, `npm test` (13/13): clean.
- Scan: 50 sentences per book taken from `pdftotext`, each searched in full and as a 7-word middle piece
  with `git grep -F --untracked` (B2-samples excluded, as they leave git): 500 searches, 0 hits. A 7-word
  shingle scan of every file left only titles, file names and short test needles.

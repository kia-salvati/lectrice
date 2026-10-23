# T: the cleaner on the user's real books (done 2026-10-23)

Only `backend/` was changed. No route, JSON field or error changed. **Cleaner version: 1 → 2**, so books saved
with version 1 are cleaned again the next time they are opened (`DocumentPipeline` already does this).

## 1. Fixture tests no longer write files
- `cargo test -- --ignored` used to rewrite `lectrice-domain/tests/fixtures/*.jsonl`. Now the ignored test
  (`lectrice-infra/tests/pdf_fixtures.rs`, `cleaner_fixtures_match_the_books`) only **checks** that the fixtures
  still match the books. It fails with a hint if they don't.
- To rewrite them on purpose: `LECTRICE_REGEN_FIXTURES=1 cargo test -p lectrice-infra --test pdf_fixtures -- --ignored`
  (documented in `backend/README.md`). `B2.md` still names the old command.
- Checked: md5 of the 5 fixtures before and after `cargo test -- --ignored` is the same, and no other file in
  `backend/` changed during the run.

## 2. What I read and what I found
I dumped every book with `lectrice-api clean` and read a beginning, middle and end chapter of each:
K8s ch 1, 8, 17 + glossary/outro · DDS ch 1, 6, 12–13 + colophon · CA ch 1, 22, 34 + appendix end ·
GoF ch 1, Decorator, ch 6 + glossary + appendix C + bibliography end · LinkedIn p8–17, p200–214. Then I diffed
the whole books before and after each fix.

| # | Problem (books) | How bad for listening | Status |
|---|---|---|---|
| 1 | **Inline code that wraps to the start of a line made the line a code block**, e.g. `…on port` / `8080.` → "Code example." in place of "8080", and the sentence cut in two (K8s ~40 places, CA ~20, DDS 1). Words around inline code lost their spaces (`toshield.mcu.comormcu.com`). | High: lost words in mid-sentence | **Fixed** |
| 2 | **A sentence starting "Figure 8.2 shows …" was taken for a caption** and not spoken (K8s 8, DDS 1, CA 1). | High: whole sentences lost | **Fixed** |
| 3 | A bold word or `Figure 1.2. That…` wrapped onto its own line became a **heading or caption** (K8s `#### svc-shield`, CA "Figure 1.2. That …" silent, K8s a bold three-word warning as `####`). | High | **Fixed** |
| 4 | **Short bare domains were silent** (B2 known problem 3): a sentence like "Traffic for ⟦shop.example.com⟧ goes to…" was spoken as "Traffic for goes to". K8s ch 8 had dozens. | High in K8s | **Fixed** (see decision below) |
| 5 | Lines mostly in inline code (`… and kubectl rollout …`) broke the paragraph, because their size looked different. | Medium: extra pauses, split sentences | **Fixed** |
| 6 | **LinkedIn labels ran into their text**: a bold label like "1. New York Times" ran straight into the first sentence of its text (B2 known problem 7). ~250 items. | Medium: no pause between title and text | **Fixed** |
| 7 | LinkedIn `1 - Database Indexing 2 - Caching 3 - …` ran together; `N - ` items whose next line has no hanging indent were cut. | Medium | **Fixed** |
| 8 | GoF glossary (double-spaced) and some GoF paragraphs: every line its own paragraph ("An abstract" / "class defers…"). | Medium: a pause after every line | **Fixed** when the next line goes on in lowercase |
| 9 | Centered dedications cut after the first line (K8s p4 and CA p6: the rest of each centered line was lost). | Low | **Fixed** |
| 10 | Mastodon handle left a spoken "@"; `linkedin.com/in/x/` left a spoken "/". | Low | **Fixed** |
| — | Checked and fine: running headers/footers and page numbers are gone in all 5; page labels right (K8s/DDS embedded, GoF = PDF page, CA/LinkedIn none); TOC and index pages dropped; hyphenation across lines and pages joined (DDS `appropri‐`/`ate` across p74→75); footnote markers silent; citations `[AIS+77]` and page refs `(349)` silent; no garbage characters. | | |

### The new general rules (no book-specific switches were needed)
- **Code lines** must be set in a code font throughout: a line with any non-monospace piece (even the `.` after
  `8080`) is prose.
- **Inline-code style** (profile): a monospace (family, size) whose chars sit mostly on lines that also hold prose.
  K8s 8.97 pt, CA 12.0 pt, DDS 10 pt. A *lone* code line in that style is prose; two or more in a row stay a
  listing (CA sets some short listings in that style). A prose line in that style takes its prose's size.
- **Captions**: the word after `Figure N` starting in lowercase means a sentence, not a caption.
- **Continuations**: a would-be heading (bold, body size) or caption right under a prose line that doesn't end
  a sentence, at paragraph pitch, is part of that paragraph.
- **Lists**: a first line with room left for the next line's first word is a label (ends the item); `1 - ` is
  a numbered item; an item goes on without a hanging indent when the next line continues in lowercase.
- **Paragraphs**: a full line that stops mid-sentence joins a lowercase next line up to 2 pitches below;
  a centered line that carries on a full first line joins it.
- **Speech**: bare domains up to 30 chars (`shield.mcu.com`, `informit.com/register`, `linkedin.com/in/x`)
  are spoken; `http(s)://` and `www.` URLs, longer bare links and e-mails (with a leading `@`) stay silent.
  **Decision for you:** this changes survey fixture 22 ("informit.com/register is not spoken"); I changed that
  test. Say if you'd rather keep all bare domains silent.

### Effect on the books (`pdf_books` test, release)
| book | blocks | sentences | code blocks | captions | clean ms |
|---|---|---|---|---|---|
| Clean Architecture | 2266 → 2238 | 5566 → 5519 | 51 → 31 | 91 → 88 | 67 |
| Designing Distributed Systems | 932 → 927 | 2402 → 2396 | 99 → 98 | 49 → 44 | 29 |
| Design Patterns (GoF) | 3559 → 3453 | 8137 → 8031 | 302 | 19 | 57 |
| LinkedIn Posts 2024 | 2616 → 2781 | 3694 → 3857 | 0 | 0 | 20 |
| The Kubernetes Book | 3380 → 3361 | 5974 → 5902 | 358 → 318 | 83 → 75 | 64 |

### Tests
- New `lectrice-domain/tests/cleaning_step_t_regressions.rs`: 11 tests, one or more per fix, on pages added to
  the fixtures (K8s 4, 99, 121–124; DDS 153; GoF 397; CA 6, 32, 94, 169, 200; LinkedIn 212, 222; fixtures now
  376 KB). Plus unit tests for caption sentences, `N - ` lists and the domain rule.
- `cargo build`, `cargo test` (297 pass, 11 ignored), `cargo test -- --ignored` (11 pass, fixtures unchanged),
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`: all pass.

## 3. End to end (release server, scratch data dir, port 7979, 8 threads)
| Check | Result |
|---|---|
| Import all 5 (`POST /api/documents`) | all 201, `cleanerVersion` 2; CA 0.79 s, DDS 0.20 s, GoF 0.71 s, LinkedIn 0.65 s, K8s 0.42 s. Sentence counts match the dumps. |
| First 20 spoken sentences of each book | 100/100 valid WAV, 24 kHz mono. Shortest 0.59 s, longest 16.4 s; 0.054–0.059 s of audio per char in every book (no empty, no huge clips). Wall time per book 17–51 s (the other books' read-ahead was running at the same time). |
| Read-ahead | Each request planned ~600 s (10 min) ahead. Cached audio: LinkedIn 721 s, K8s 704 s, GoF 714 s, DDS 741 s (20 heard + ~10 min ahead each). CA stopped at 371 s: it was the oldest of 5 open books and read-ahead keeps 4 plans (`MAX_PLANS`), so its queued jobs were cancelled, as designed. |
| M4B export, K8s | `LECTRICE_EXPORT_MAX_SENTENCES=535` (the whole book would take hours; export only cuts from the start), so the file holds the front matter, ch 0, ch 1 and **all of ch 2 "Kubernetes principles of operation" (23 min)**, plus the first minute of ch 3. Took 14 min while read-ahead shared the CPU. 202 → running → done 535/535, time left shown from the first synthesized sentence. Download `audio/mp4`, 20.1 MB, AAC 24 kHz mono, **36:54**. ffprobe: **21 chapters** in order, from the headings ("0: Preface", "1: Kubernetes primer", "Important Kubernetes background", … "2: Kubernetes principles of operation" at 12:58, "Pods", "Chapter summary", "3: Getting Kubernetes" at 35:54). |
| Saved for you | `docs/progress/T-k8s-chapter.m4b` (`xdg-open` it) |
| Server | stopped cleanly ("Server stopped"). |

## 4. Review samples regenerated
`docs/progress/B2-samples/*.txt` now come from cleaner v2 (same page ranges: CA pdf 52–64, DDS 25–34,
GoF 99–109, K8s 222–235, LinkedIn 8–30).

## 5. Still not right (most harmful for listening first)
1. **Text inside figures and grid tables is read as short paragraphs or notes** (K8s `Pods Services Ingress
   controllers … shield svc-shield`, GoF Table 1.1 as "Purpose", "Creational Structural Behavioral"…). Needs
   figure regions or real table structure (phase 2).
2. **GoF's missing spaces come from the PDF** (`weredescribing`, `Afterall`, `aVisitor&`), mostly in the
   prefaces, ch 6 and the appendices. The voice will slur these words. Left as is (your earlier decision).
3. **Tables are only "Table."**: the whole K8s glossary (~7 pages) is skipped when listening unless "Read code
   & tables aloud" is on.
4. A lone one-line listing in CA's inline-code style is read aloud (a one-line C function declaration, rare).
5. Some splits remain where the next line starts with a capital or a symbol: GoF glossary entries where the next
   line starts with a capital letter, LinkedIn lines that go on with a `$` price.
6. Hyphens inside wrapped inline code stay: `PodSecuri-tyContext`, `readOnlyRoot-Filesystem` (K8s).
7. Front matter: "Tweet This Book!" is spoken and is the first M4B chapter; title pages give small headings.
8. Sentence splitting: `U.S.` never ends a sentence; `10²²` is shown but not spoken (B2 known 8, 9).
9. View only (silent anyway): DDS copyright lines labelled as footnotes; GoF bibliography keys as headings.

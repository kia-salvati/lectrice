# Build brief · "Play from an audiobook file" UI options (Opus → Fable, 2026-11-13)

Spec: `docs/android-logic/23-audiobook-source.md` (read §2, §4, §3.7). Output: **one local HTML file**
`design/2026-11-13-audiobook-source-options.html` (never published). Phone frames 412 × 892 dp like
`design/2026-10-23-android-quiet-v2.html`; **copy its tokens, type, components and phone frame exactly** (Quiet v2 is the
approved look: pill, player sheet, Listen view, library, E4 export page). Light and dark side by side for every screen.
No visible scrollbars. All text invented (book "The Long Book", 14 chapters, voice Heart; never real books).

Top of the page: a short intro, then the three options as sections A, B, C, each with its screens in a row, a
"For / Against" list (given below, copy it), and at the end a section "Shared by all options" (screens S1–S5).
Mark option **B as "Recommended"** with a small badge.

## Shared screens (identical in A, B, C)

- **S1 · Export done (E4)** as in Quiet v2's E4, plus a new primary-tonal button **"Listen from this file"** under the
  title block, above "Save to Audiobooks". Under it, caption: "The text follows each sentence."
- **S2 · Library import menu**: the round "Open PDF" button opens a small menu (Quiet v2 menu style) with two rows:
  "PDF book" (icon `picture_as_pdf`) and "Audiobook file (M4B)" (icon `headphones`).
- **S3 · Linked snackbar** on the library: "Linked to *The Long Book*." action **Listen**.
- **S4 · Other-book sheet** (bottom sheet): title "This audiobook belongs to another book", body "It was made from
  *A Quiet Harbour*. Open that PDF to read along.", buttons **Open PDF…** (filled) and **Not now** (text).
- **S5 · Which-book sheet** (foreign file): title "Which book is this audiobook for?", subtitle "Made by another app, so
  the text will follow chapters, not sentences.", list of 3 library rows with cover tiles; the first has a caption
  "12 of 14 chapters match" in accent colour; others "No chapters match". Text button **Cancel**.

## Option A · "Inline switch"

- **A1 · Player sheet** (half height, Quiet v2): new row under Voice: label "Audio", and a 2-segment control
  **Made on this phone · Audiobook file** (Audiobook selected). Under it one caption line:
  "2 h 41 min · Heart · made at 1.5× · text follows each sentence". Voice row disabled (60 % opacity) with caption
  "From the audiobook".
- **A2 · Pill in file mode**: the collapsed and expanded pill with a 16 dp `headphones` mark before the status text
  ("Ch. 3 · p. 41").
- **A3 · File missing**: the same sheet row with a warning banner (Quiet v2 warn banner) "Audiobook file missing" and
  two text actions "Find it again…" / "Use Made on this phone".
- For: one tap to switch; everything in one place. Against: the caption line gets crowded; little room for the
  chapter-mode note and errors; the segmented control is tight at 200 % font.

## Option B · "Audio source sheet" (Recommended)

- **B1 · Player sheet**: new list row under Voice (same style as the Voice row): leading icon `headphones`, title
  "Audio", subtitle "Audiobook file · text follows each sentence", trailing chevron. Voice row disabled as in A1.
- **B2 · Audio source sheet** (opens from B1; bottom sheet, half height): title "Listen with". Two selectable cards
  (radio, like the model cards S2): **Made on this phone** ("Heart · your speed · works for every page") and
  **Audiobook file** (selected; lines: "The Long Book – Heart.m4b", "2 h 41 min · made at 1.5× · from this phone",
  "Text follows each sentence"). Under the cards, text buttons "Use another file…" and "Remove link" (error colour).
- **B3 · Pill in file mode**: as A2.
- **B4 · File missing**: B2 with the Audiobook card in warn style: "File missing" and a button "Find it again…".
- **B5 · Chapter mode**: B2 where the Audiobook card says "Made by another app · text follows chapters" with an
  info icon.
- For: room for status, chapter-mode and error text; the sheet row stays one line; scales to a third source later
  (file 24's computer helper shows as a status line, not a source). Against: two taps to switch.

## Option C · "Source chip"

- **C1 · Pill in file mode**: the expanded pill gets a small chip at its left "File" (outlined, 28 dp) next to ⏮;
  tapping the chip toggles the source with a snackbar "Listening from the audiobook" / "Making speech on this phone".
- **C2 · Listen view**: the top bar shows the same chip next to the section title.
- **C3 · Player sheet**: no new row; Voice row caption "From the audiobook file".
- For: fastest switch, always visible. Against: an easy accidental toggle; no place for errors or chapter mode; the
  pill is already full at 200 % font.

## Notes

- Icons: Material Symbols Rounded names as in Quiet v2 (`headphones`, `picture_as_pdf`, `warning`, `info`, `chevron_right`).
- Keep it static HTML/CSS (no framework). Screens labelled with their ids (A1…C3, S1…S5).
- When done, take light/dark screenshots of each option with headless Chrome into `design/screens/audiobook-*.png`.

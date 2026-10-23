# Lectrice: PDF survey for the cleaner (2026-10-21)

Scope: look at the 5 real PDFs on the Desktop and find the patterns the PDF cleaner must handle. This is research only. No app code was written. No PDF was changed.

Method: `pdfinfo` and `pdftotext` (plain and `-layout`), plus **pypdfium2** in a throwaway venv in the scratchpad. pypdfium2 uses the same PDFium engine as the Rust `pdfium-render` crate. For each character I read the unicode value, font name (`FPDFText_GetFontInfo`), font size (`FPDFText_GetFontSize`), char box, `IsGenerated` and `IsHyphen`. Then I grouped the characters into lines. I also ran whole-book stats (font/size histogram, first and last line of each page, odd characters, page labels, outline) on every page of every book. I looked closely at 6 to 12 pages per book.

Excerpts below are raw PDFium output. `y=` is the baseline in PDF points (0 = bottom of page), `x=` is the left edge, then font size and font name. Page numbers are **PDF page index (1-based)** unless I write "label".

Anything marked **UNVERIFIED** was not checked.

---

## 0. Engine facts that matter more than any one book (verified with pypdfium2)

1. **PDFium marks line-end hyphens.** If an ASCII `-` ends a line, PDFium gives the char unicode `0x0002` (`FPDFText_GetUnicode`). `FPDFText_GetText` turns it into `U+FFFE`, `FPDFText_IsHyphen` returns 1, and PDFium **removes the line break** after it. So the next line is glued on: `Unfortu￾nately`, `billion￾dollar`. PDFium does **not** know whether the hyphen is a real one (`billion-dollar`) or a syllable break (`Unfortu-nately`). The cleaner must decide. pdfium-render's char API probably exposes this as a `0x02`/`U+FFFE` char. **UNVERIFIED** in pdfium-render: the exact accessor names, and whether it exposes `is_hyphen`.
2. **The Unicode hyphen U+2010 (`‐`) is NOT marked.** The line break stays. Only DDS uses it.
3. **Char order is not reading order.** GoF: the footer page number comes 2nd, right after the running header and before the body. DDS p23: the footnote comes before the body. DDS p4: the ISBN at the bottom comes first. So headers, footers and footnotes must be found **by position**, never by "first line / last line".
4. **`FPDFText_GetFontSize` gives the nominal size, not the size you see.** Calibre (CA) reports body text as 20.0, but it looks like about 13pt (the line pitch is 17.3pt). The DDS cover reports 1.0. **Only compare sizes inside one document** (ratios to the body size). Never use absolute thresholds.
5. **Generated spaces** (`IsGenerated=1`) have font size 1.0 and an empty box. Leave them out of every size statistic.
6. **Superscripts cause a line break.** PDFium puts `\r\n` before a raised, smaller run (CA p20: `least 10\r\n22 more powerful`). Line building must merge runs that overlap vertically and sit next to each other horizontally.
7. **Leading spaces are dropped.** Code indentation exists only as the x offset (GoF p69: `x=111.6` vs `x=155.3`). To show code in the view, rebuild the indent from `(x - block_left) / char_width`.
8. **Page labels and outline.** pypdfium2 `get_page_label` and `get_toc` work. `pdfinfo` does not show labels.

---

## 1. Per-book findings

### 1.1 Clean Architecture (CA): 429 pages, calibre 3.7 (an EPUB printed to PDF)

**Text layer:** good. Real text, no OCR. 48 pages have no text (cover p1, p3, and the image-only "Code Snippets" pages at p375–429).

**Headers/footers/page numbers:** **none.** It is a reflowed e-book, so there are no running heads or printed page numbers. **No embedded page labels.** The outline is good (226 items: parts, chapters, sections).
Leftover print page numbers show up twice as `{xx}` and `{xxiii}`:
```
p20  'machines.{xx}'
p22  '{xxiii}'           (on a line by itself)
```
The index (p335–374) points to *print* page numbers that do not exist in this PDF: `'BOSS, 351–352'`.

**Fonts** (share of chars): TimesNewRomanPSMT 20.0 = 90.7% (body). 17.0 = block quotes (italic), captions, footnotes and code all share this size. Courier 16/17 = code. Bold 33 = chapter title and H1. Bold 26 = H2. Bold 40 = chapter number.
```
p27  y=630.3 x=300.5 40.0 Bold | '1'
p27  y=596.1 x= 55.5 33.0 Bold | 'WHAT IS DESIGN AND ARCHITECTURE?'
p28  y=406.1 x= 50.9 33.0 Bold | 'THE GOAL?'
p30  y=111.5 x= 50.7 26.0 Bold | 'THE SIGNATURE OF A MESS'
p28  y=321.3 x= 68.9 17.0 Italic | '(block-quote line, 92 chars)'
p29  y=410.5 x= 50.3 17.0 Roman+Bold | 'Figure 1.1 Growth of the engineering staff'
```
Headings are ALL CAPS in the text, but the outline has them in normal case: `Chapter 1 What Is Design and Architecture?`, `The Goal?`.

**Footnotes:** the marker is a raised digit at size 15 (body 20). The note is at size 17 and starts with `N. `:
```
p21  y=724.3 20.0 | '(body line, 78 chars, ending in "put her")'
p21  y=732.8 x=532.9 15.0 | '1'                 <- marker, raised ~5pt
p21  y= 85.0 x= 51.5 17.0 | '1. (footnote text, 90 chars)'
p74  y=457.0 17.0 | '1. For example, (a book title, its author),'
p74  y=442.0 17.0 | '(publisher, year, a long URL), and'
```
**Trap:** a math exponent looks exactly like a footnote marker:
```
p20  '...at least 10'  then  '22 more …'   ('22' = size 15, raised 5.4pt -> it's 10^22)
```

**Code:** Courier at size 16/17, with a link label before each listing:
```
p53  y=358.0 17.0 Bold    | 'Click here to view code image'
p53  y=325.4 17.0 Courier | '<file name>.h'
p53  y=297.7 17.0 Courier | '<a one-line C declaration>'
p53  y=282.4 17.0 Courier | '<a C function prototype, 44 chars>'
```
"Click here to view code image" appears 36 times on 23 pages. It is junk in both speech and view.
Inline code inside body lines uses Courier inside a Times line: a body sentence that names the header file and the struct in Courier.

**Hyphens:** all 36 PDFium-marked hyphens are **real** compound hyphens (calibre does not hyphenate words): `co￾founder`, `billion￾dollar`, `ever￾increasing`, `high￾level`.

**Odd characters:** no ligatures. One broken word: `p20 '… stuf .'` (the second `f` is missing, and a generated space is there instead). Bullets are `•`.

**Lists:** `• “Slow and steady wins the race.”`, and numbered lists `1. Urgent and important`. These look like footnotes ("N. "). Tell them apart by size (17 vs 20) and position (bottom of page).

**Back matter:** Index p335–374 (single column, sub-entries indented at x≈65). Code Snippets p375–429 (images only).

### 1.2 Designing Distributed Systems (DDS): 164 pages, O'Reilly / Antenna House

**Text layer:** excellent.

**Page labels:** embedded and correct: `Cover, BackCover, i … xii, 1 … 150`. pdf p15 = label "1". Outline: 139 items.

**Footers (no headers):** bottom band at y≈40.5. Even and odd pages are different:
```
p26 (label 12, even)  y=40.5 x= 72.5  9.0 MyriadPro-SemiboldCond | '12 | Chapter 2: The Sidecar Pattern'
p27 (label 13, odd)   y=40.5 x=296.7  9.0 MyriadPro-SemiboldCond | 'Dynamic Configuration with Sidecars | 13'
p15 (chapter start)   y=42.8 x=428.8  9.0 MyriadPro-SemiboldCond | '1'
p9  (front matter)    y=42.8 x=425.0  9.0                        | 'vii'
```

**Fonts:** MinionPro 10.5 = body (79%). UbuntuMono 8.5 = code blocks (7%). UbuntuMono 10 = inline code. MinionPro 9.6 = note/tip boxes. MinionPro 9.0 = index. MinionPro 8.0 = footnote. MyriadPro-SemiboldCond: 25.2/28.4 chapter/part title, 18.9 H1, 15.8 H2, 12/14 TOC.
```
p15  y=569.6 16.8 Myriad | 'CHAPTER 1'
p15  y=539.6 25.2 Myriad | 'Introduction'
p21  y=567.7 18.9 Myriad | 'PART I'
p21  y=528.9 28.4 Myriad | 'Single-Node Patterns'
p25  y=166.8 10.5 MinionPro-It | 'Figure 2-1. The generic sidecar pattern'     <- caption = body size, italic!
p18  y=468.7 x=136.9 9.6 MinionPro | '(note-box line, 64 chars, a question)'   <- note box, indented
```

**Code:**
```
p29  y=500.4 x=89.5 8.5 UbuntuMono | '$ docker run --pid=… \\'
p29  y=490.2 x=89.0 8.5 UbuntuMono | ' -p 8080:8080 \\'
p29  y=125.3 x=89.2 8.5 UbuntuMono | '#!/bin/bash'
p29  y= 94.6 x=89.0 8.5 UbuntuMono | ' git pull'
```

**Hyphens (very important here):** O'Reilly uses **U+2010 `‐`** for syllable breaks at line end. There are 390 of them, and **all 390 are at a line end**:
```
'…back‐' / 'end …'      -> backend
'…solu‐' / 'tions.'      -> solutions
'…engi‐' / 'neering …'  -> engineering
```
Real compound words keep ASCII `-`, and PDFium marks them when they fall at a line end: `object￾oriented`, `time￾sharing`, `pre￾existing`, `Addison￾Wesley`.
So in DDS the rule is easy: `‐` at line end means join with no hyphen, and `￾` means keep `-`.

**Footnotes:** only one (p23). The marker is `1` at size 6.0, raised. The note is at size 8.0 at the bottom and comes **first** in char order:
```
p23  y= 85.5 8.0 | '1 (footnote: a one-sentence definition, 105 chars)'
p23  y=558.4 10.5 | 'a pod in Kubernetes,1'
```

**Index:** 2 columns (x=72 and x=259). PDFium order goes column by column, which is good: `'adapter patterns, 31-39'`, a sub-entry wrapped with `con‐` / `tainer, 36-39`.

**Copyright p4:** all text at 8.5, with ISBN and `[LSI]` first in char order.

### 1.3 Design Patterns, Gang of Four (GoF): 431 pages, pdfFactory

**Not a scan and not OCR.** It is a real text layer, printed from an HTML/CHM edition ("Produced by KevinZhang", p1). **Every glyph is Courier New:** body, headings, code, everything (CourierNew 92%, Bold 6%, Italic 2%). "Monospace font = code" is **wrong** for this book.

**Running header on 430/431 pages** at y=742.5, 8.4 Bold:
```
'Design Patterns: Elements of Reusable Object-Oriented Software '
```
**Page number** in the footer, y≈63.2, x≈480–495, **13.2 Bold. That is the same size and weight as section headings.** In char order it comes 2nd, before the body:
```
p11  y=742.5 x=187.6  8.4 Bold | 'Design Patterns: Elements of Reusable Object-Oriented Software '
p11  y= 63.4 x=487.3 13.2 Bold | '11 '
p11  y=704.3 x=245.3 15.0 Bold | '1. Introduction '
```
**No page labels**, but the printed number equals the PDF index (p11 prints "11"). The TOC numbers also match the PDF index (`Preface to CD .... 5` → pdf p5). The outline has 392 items.

**Fonts:** body 8.4. **Code is 7.6, which is smaller than body** (9.2% of chars, 162 pages). Pattern title 15.0 Bold. Section 13.2 Bold. Pattern-part labels (`Intent`, `Motivation`, `Applicability`) are 13.2 Bold or 8.4 Bold.
```
p69  y=583.8 x=111.6 7.6 | '<C++ method signature, 72 chars> '
p69  y=554.8 x=155.3 7.6 | '<an indented statement, 28 chars> '
p69  y=669.8 x=111.6 8.4 | '(body line, 77 chars, ending in "stores theWindowImp with which") '
```
**Every line ends with a trailing space.** There is no hyphenation and there are no ligatures.

**Missing spaces (a text-layer defect):** many joined words, mostly in later chapters:
```
p69   'stores theWindowImp with which', 'subclassthat', 'becauseXDrawRectangle', 'width,and', 'valuesfrom'
p413  'Cartesiancoordinate', 'arithmetic.The'
p416  'Hierarchyevolution', 'ConferenceProceedings', 'Angel.A Pattern Language', 'NewYork'
```
A dictionary check finds clear joins like `Forexample`(10), `thedocument`(9), `theclasses`, `anobject`, `withthe`, `theyare`. The pattern "function word glued to a Capitalized word" (`theRequest`, `aWindowImp`) appears about 275 times on 118 pages, but some of these are real identifiers (`aFactory`). So it is **not safe to auto-split in v1.**

**Citations:** `[AIS+77]`, `[KP88]`, `[RBP+91]`, `[JCJO92, Boo94]`. That is 232 hits on 90 pages, with alphanumeric keys, not `[12]`.
**Cross-refs:** `Visitor (page 366)`, `Iterator (289)`, `Abstract Factory (99)`. These are page numbers inside the prose.
**Bullets:** `·` from the Symbol font (268 times): `'· (list item, 75 chars) '`.
**Figures:** images only. You see a big y-gap in the text (p15: the text jumps from y=684 to y=334).
**Table 1.1 (p21):** cells come out as separate short lines at different x values: `'Purpose'`, `'Creational Structural Behavioral'`, then `-layout` shows a grid. Caption: `Table 1.1: Design pattern space`.
**Bibliography p416–431:** `'[AG90]'` then the entry. No index.

### 1.4 LinkedIn Posts 2024 (LI): 368 pages, Google Docs (Skia), tagged PDF

**Structure:** it is not a book. It is about 180 short posts. Each post = a heading (Arial-Bold 21.3) + a large infographic image (no text, the text is inside the image) + body text (Arial 14.0) + bullets. Pages p1 and p368 are image only.

**No headers, footers or printed page numbers. No page labels.** The outline has 215 items, and some are junk: an empty title, and body sentences used as titles (a whole body sentence of about 70 chars).
**TOC p3–7** (Arial 14.7) with numbers equal to **pdf page − 1** (`'Big Endian vs Little Endian 7'` → pdf p8). The TOC is broken in one place, where a post body leaked into it:
```
p5  'How does gRPC work?'
p5  '(post body line with a styled word like 𝐫𝐞𝐦𝐨𝐭𝐞, 80 chars)'
p5  '(post body line, 80 chars) 147'
```
**Post page:**
```
p8   y=701.6 21.3 Arial-Bold | 'Big Endian vs Little Endian'
p8   (gap y=700 -> 141: image)
p8   y=140.9 14.0 Arial      | '(body line, 98 chars)'
p8   y=108.8 x=90.9 14.0 Arial-Bold | '● Little Endian'
p10  y=216.1 x=91.1 14.0 | '1. New York Times'
```
**Odd characters (LinkedIn styling):** "Unicode bold/italic" letters, 60+ kinds: `𝐫𝐞𝐦𝐨𝐭𝐞`, `𝐦𝐞𝐫𝐠𝐞 𝐜𝐡𝐚𝐧𝐠𝐞𝐬`, `𝘛…`. These are Mathematical Alphanumeric Symbols, and **NFKC turns them into plain ASCII**. Emoji bullets: `🔹` (259), `●` (86), `▶️` (10, with the variation selector U+FE0F). These come from fallback fonts (FreeSerif, Andika, Roboto, and an empty font name).
No URLs. No code blocks. Calls to action like `Over to you: …` followed by a question are content, so keep them.

### 1.5 The Kubernetes Book 2024 (K8s): 308 pages, Leanpub / XeTeX

**Text layer:** excellent.

**Page labels:** `i–iv` (pdf 1–4), **empty** for the TOC (pdf 5–8), then `1…300` (pdf 9 = "1", so offset 8). Outline: 127 items, good.

**Running header** at y≈601.6–603.6, CrimsonText 8.0, the same pattern on odd and even pages: `"<chapter>: <title> <label>"`. There is no header on chapter-start pages. **No footer.**
```
p12 (label 4)   y=601.6 x=54.9 8.0 | '1: Kubernetes primer 4'
p60 (label 52)  y=601.6 x=54.0 8.0 | '4: Working with Pods 52'
p307            y=603.6        8.0 | 'Outro 299'
p300 (pdftotext)                    '292'      (appendix: number only)
```

**Fonts:** CrimsonText 10.0 = body (73%). SourceCodePro 7.0 = code blocks (14%, smaller than body). SourceCodePro-Bold 9 = inline code. CrimsonText 9.0 = tables and glossary. 8.0 = header, footnotes **and** figure captions (Bold). OpenSans-Bold: 20.7 chapter, 14.3 H1, 12.0 H2.
```
p11  y=526.4 20.7 OpenSans-Bold | '1: Kubernetes primer'
p11  y=407.8 14.3 OpenSans-Bold | 'Important Kubernetes background'
p11  y=331.8 12.0 OpenSans-Bold | 'Orchestration'
p14  y=167.7 x=233.9 8.0 CrimsonText-Bold | 'Figure 1.2'
p121 y=127.5 x=196.8 8.0 CrimsonText-Bold | 'Figure 8.2 Host-based routing'
```

**Footnotes** (14 of them, almost all are just a URL). The marker is a raised digit at 6.97 glued to a word. The note is at 8.0 at the bottom, with the number glued to it:
```
p13  y=273.3 10.0 | 'Cloud Native Computing Foundation (CNCF)1'      ('1' size 6.97, raised 3.6pt)
p13  y= 62.8 x=68.9 8.0 | '1https://www.cncf.io'
p36  '...as it’s free3'   ->  y=113.0 8.0 '3(footnote about licensing, 100 chars)'
```

**Code and terminal output:**
```
p60  y=534.1 x=54.7 7.0 SourceCodePro | 'kind: Pod'
p60  y=472.5 x=63.0 7.0 | '- name: app <<==== (comment)'
p60  y=392.2 x=54.8 7.0 | '<Snip>'
p61  y=570.7 7.0 | '$ kubectl exec hello-pod -- ps'
p61  y=562.0 7.0 | 'PID USER TIME COMMAND'
p61  y=550.5 7.0 | '1 root 0:00 node ./app.js'
p68  y=519.8 7.0 | 'https://github.com/<author>/<repo>'
```

**Hyphens:** 133 PDFium-marked hyphens on 105 pages, **mixed**:
- syllable breaks: `Unfortu￾nately`, `Win￾dows`, `contain￾ers`, `im￾mutable`
- real hyphens: `first￾choice`, `cloud￾native`, `self￾healing`, `hello￾pod` (a Pod name!)

**Tables:** each row is collapsed into one line, and cell borders are lost. Wrapped cells continue on the next line at the column-2 x:
```
p228 y=430.7 x=99.9  9.0 Bold | 'Kubernetes verb(s) HTTP method Common responses'
p228 y=420.1 x=100.0 9.0      | 'create POST 201 created, 403 Access Denied'
p299 y=421.8 x=59.5  9.0      | 'Admission controller (definition, 55 chars)'
p299 y=408.6 x=216.1 9.0      | '(rest of the definition, 50 chars)'
```
**Notes:** body size, indented x≈79, start with `Note:` or `Warning.` (21 of them). Keep them.
**Back matter:** p299–305 Terminology (a table). p307 Outro. **p308 "Index" has a heading only, no entries.** No bibliography.
**Odd characters:** `∼` (CMSY10, "∼10 years old"). Box-drawing chars `├ └ ─` appear in tree output inside code.

---

## 2. Cross-book pattern table

| Pattern | CA | DDS | GoF | LI | K8s |
|---|---|---|---|---|---|
| Producer | calibre (EPUB→PDF) | Antenna House | pdfFactory (HTML→PDF) | Google Docs | XeTeX |
| Running header | none | none | **every page**, y=742.5, fixed text | none | y≈602, "N: Chapter P" |
| Footer | none | y≈40.5, even/odd differ | page no. y≈63, 13.2 Bold | none | none |
| Printed page no. | none (`{xx}` leftovers) | in footer | in footer (= pdf index) | none | in header |
| Embedded page labels | no | **yes**, correct | no | no | **yes** (TOC pages empty) |
| Outline | good | good | good | partly junk | good |
| Body font / size | Times 20 (nominal) | Minion 10.5 | **Courier** 8.4 | Arial 14 | Crimson 10 |
| Code font / size vs body | Courier, smaller (17) | UbuntuMono, smaller (8.5) | **Courier, smaller (7.6), same font as body** | none | SourceCodePro, smaller (7.0) |
| Headings | bold, larger, ALL CAPS | Myriad, larger | Courier Bold larger | Arial Bold 21.3 | OpenSans Bold larger |
| Captions | 17 (smaller), "Figure 1.1" | **body size**, italic, "Figure 2-1." | "Table 1.1:" | none | 8.0 Bold, "Figure 8.2" |
| Footnotes | raised marker + "N. text" at 17 | 1 only, 8.0 | none | none | raised marker + "Ntext" at 8.0 |
| Citations | rare | none | **[AIS+77] style, 232** | none | none |
| URLs | in footnotes/front matter | some inline | none | none | 54, many in footnotes/code |
| Line-end hyphen type | `￾` = real | `‐` = break, `￾` = real | none | none | `￾` = **mixed** |
| Ligatures (ﬁ ﬂ) | none | none | none | none | none |
| Smart quotes | yes | yes | mostly straight `"` `'` | yes | yes |
| Multi-column body | no | no (index = 2 col) | no | no | no |
| Tables | rare | rare | some (grid of short lines) | none | several (rows collapsed) |
| Image-only pages | 48 | covers | none | 2 | 1 |
| Text defects | 1 lost letter (`stuf`) | none | **missing spaces** | NFKC-able styled letters, emoji | none |

**Ligatures:** none found in any book (0 of U+FB00–FB04). The ligature rule is cheap insurance only.
**Soft hyphen U+00AD:** none found.
**True two-column body text:** none. "Fix multi-column reading order" matters only for the DDS index (which we skip anyway) and for tables.

---

## 3. Answers to the key questions

**Is "drop text below the median body size" safe? No.** It would delete:
- all code in DDS, GoF and K8s (code is smaller than body in all three),
- CA block quotes, which are core content (an italic quote at the start of chapter 1, size 17 vs 20),
- DDS note/tip boxes (9.6 vs 10.5),
- K8s tables and the Terminology appendix (9.0 vs 10.0),
- captions in CA and K8s (we may want these in the view).

It would **miss** DDS captions (body size) and GoF page numbers (larger than body). Size is a useful *signal* only when combined with position (bands at top/bottom), pattern (digits, `Figure N`) and font family.

**Code / terminal output policy (recommended):**
- **Detect** a block of 2+ consecutive lines where (a) the font is a monospace family that is *not* the body font (Courier, Mono, Code, Consol, Menlo; this is book-relative, so GoF's Courier body is excluded) **or** (b) for a monospace-body book (GoF), size < body by ≥ 5%, **plus** code-shape signals: lines start with `$ `/`# `, contain `{ } ; ( ) = :` or `<Snip>`, YAML `key: value`, or have deep x-indent. A single monospace line between body paragraphs (`$ kubectl logs <pod> --container <name>`) is also a code block.
- **View:** show it as a code block (monospace, indent rebuilt from x).
- **Speech:** **skip**, and say a short cue once per block, for example "Code example." (or "Command." if the block starts with `$`). Reasons: (1) TTS reading `apiVersion: apps/v1` or `{ _imp->Draw(x0, y0); }` is noise and is slow to listen to. (2) The prose around the code nearly always explains it ("Run the following command…"). (3) The cue tells the listener to glance at the screen. Make "read code aloud" a per-book toggle (off by default).
- **Inline code** inside a body line (`kubectl exec`, `point.h`): keep it and speak it as normal text.

**Recommended keep/skip table:**

| Element | View | Speech | Notes |
|---|---|---|---|
| Body paragraphs | show | speak | |
| Chapter/section headings | show | speak | Use outline titles for casing when they match (CA ALL CAPS). |
| Bullet / numbered lists | show | speak | Strip the bullet glyph (`• · ● 🔹 ▶️`), keep "1." numbers. |
| Block quotes (CA) | show | speak | |
| Notes/tips/warnings (DDS 9.6 box, K8s `Note:`) | show (styled) | speak | Maybe prefix "Note." |
| Code blocks, terminal output | show | **skip + cue** | |
| Figure/table captions | show | **skip** (v1) | Option: speak "Figure 2-1: The generic sidecar pattern". Open question. |
| Tables | show (as lines/grid) | **skip + cue** "Table." | Row text collapses badly in K8s. |
| Images | (render page crop later) | skip | No text layer. LI infographics lose their content (OCR = later). |
| Running headers/footers, page numbers | hide | skip | Turn into metadata: page label. |
| Footnote markers | hide (or superscript link) | skip | Only when a matching footnote exists on the page. |
| Footnote bodies | show collapsed at block end | skip | |
| Citations `[12]`, `[AIS+77]` | show | skip | |
| URLs | show | skip | Maybe say "link" when the URL is the whole sentence object. |
| "Click here to view code image" (CA) | hide | skip | |
| TOC pages | hide (we have the outline) | skip | |
| Index | hide | skip | |
| Bibliography (GoF p416+) | show (or hide) | skip | |
| Copyright / title / "Tweet this book" / ads (LI p2) | show | skip | |
| Cover / image-only pages | placeholder | skip | |

---

## 4. Proposed v1 cleaning pipeline

Input: `TextRun { pdf_page, text, font_name, font_size, bbox }` from PDFium, in PDFium char order. Rules run in order. Each rule tags runs, lines or blocks as `Speak`, `ShowOnly` (in the view, not spoken), or `Drop` (not even shown). Confidence: **H** = seen and checked on several books, **M** = works on the samples but has edge cases, **L** = a guess.

| # | Rule | What it detects / does | Decision | Conf. | Books |
|---|---|---|---|---|---|
| 1 | **Document profile** | Over the whole document (ignoring generated chars and size < 2): body size = the most common (font, size) by char count. Body font family. `mono_body = body font is monospace`. Heading sizes = sizes ≥ 1.15× body seen on short lines. Keep everything as ratios. | n/a | H | all |
| 2 | **Page labels** | `label = pdf.page_label(i)`. If it is empty or missing, fall back to the number found by rule 4. Store `pdf_page` + `label` (only when label ≠ pdf_page) as block metadata. | metadata | H | DDS, K8s (embedded); GoF (from footer) |
| 3 | **Build lines** | Merge runs with overlapping vertical extent and small x gaps, **including** raised smaller runs (superscripts), even if PDFium put `\r\n` between them. Trim trailing spaces (GoF). Keep PDFium order between lines, *except* that rules 4/5/8 pull out position bands first. | n/a | H | all |
| 4 | **Header/footer bands** | For each page, take lines in the top 12% and bottom 10% of the page height. Normalize digits and roman numerals to `#`. A band line is a running header/footer if its normalized text (or its "page-number-only" shape `^\s*[#ivxlc]+\s*$`, `# \| …`, `… \| #`, `#: … #`) repeats on ≥ 30% of pages in that band (compare odd and even pages separately). Pull the number out as the printed label. | **Drop** (text), label → metadata | H | GoF, DDS, K8s |
| 5 | **Lone page numbers** | A band line that is only a number/roman numeral, ±1 of the expected label (chapter-start pages in DDS: `'1'` at y=42.8). | Drop → metadata | H | DDS, GoF |
| 6 | **Image-only / empty pages** | Fewer than ~5 real chars. | placeholder in view | H | CA (48), LI, covers |
| 7 | **Page classifiers** (whole page) | **TOC:** ≥ 40% of lines end with `\.{3,}\s*\d+$`, `(\. ){3,}\d+$` or a trailing number, *or* the page lies between outline items titled "Contents"/"Table of Contents" and the next item. **Index:** heading "Index" plus many lines matching `, \d+(–\d+)?` / `see also`. **Bibliography:** heading "Bibliography/References" plus `^\[[A-Za-z+]+\d+\]` lines. **Copyright:** contains `ISBN` / `All rights reserved` / `Copyright ©`. Prefer outline titles to find these sections. | TOC, Index: Drop. Bibliography, copyright, CA "Code Snippets" pages: ShowOnly | M | all |
| 8 | **Footnotes** | (a) Bottom-of-page lines with size ≤ 0.85× body, starting with `^\d{1,3}` (`1. And she…`, `1https://…`, `1 Kubernetes…`), placed below the last body line (*position*, not char order). (b) Markers: a raised (baseline +≥ 20% of size), smaller (≤ 0.8×) run of 1–3 digits at the end of or after a word. **Drop a marker only if a footnote with the same number is on that page**, otherwise keep it as a superscript (CA `10^22`). | footnote body: ShowOnly (attach to the block). Marker: hidden in speech | M | CA, K8s, DDS |
| 9 | **Code blocks** | See §3. 1+ consecutive lines in a non-body monospace font, or (if `mono_body`) a smaller monospace size with code-shape signals. Rebuild indent from x. | ShowOnly + spoken cue | H for CA/DDS/K8s, M for GoF | all but LI |
| 10 | **Known junk lines** | `^Click here to view code image$`. `\{[xivlc]+\}` leftovers. | Drop | H | CA |
| 11 | **Headings** | A short line (< ~12 words, no final period) with size ≥ 1.15× body, or a bold-family font ≠ body on its own line with space above. Merge stacked heading lines (DDS `CHAPTER 1` + `Introduction`, CA `1` + `WHAT IS …`) into one. Match against outline titles (fuzzy, case-insensitive). If they match, use the outline's casing and level. | Speak, block type = heading | M (H when outline-matched) | all |
| 12 | **Captions** | `^(Figure\|Table\|Listing\|Example)\s+\d+([.\-]\d+)*[.:]?\s` as a line on its own (any size). | ShowOnly (v1) | H | CA, DDS, GoF, K8s |
| 13 | **Tables** | 3+ consecutive lines at smaller-or-body size where lines start at 2+ repeated x columns (K8s x=59.5/216, GoF Table 1.1) or have a bold first row, often between a caption and body. | ShowOnly + cue "Table." | L | GoF, K8s |
| 14 | **Note boxes** | Lines indented ≥ 40pt more than body with size 0.85–1.0× body (DDS), or lines starting `Note:`/`Tip:`/`Warning.` | Speak (type = note) | M | DDS, K8s |
| 15 | **Lists** | Line starts with `[•·●▪◦🔹▶️➤✔-]\s` or `\d+\.\s` (and is not a footnote). Strip the glyph for speech. Continuation lines are indented to the text start. | Speak | H | all |
| 16 | **Paragraph building** | Join lines into paragraphs: break on y-gap > 1.4× line pitch, on an indent change, or when a line ends with `.?!:` and the next starts at the left margin. Join across page breaks when the last block of page N has no final punctuation and the first body block of N+1 starts lowercase (the block keeps `pdf_page` = start page and records `continues_on`). | n/a | M | all |
| 17 | **De-hyphenation** | (a) `U+2010` at line end: join, remove the hyphen. (b) `￾`/`0x02` (PDFium marker) or ASCII `-` at line end: build a book vocabulary from all words that appear *unbroken* mid-line. If `left+right` is in the vocab (or in a dictionary) and `left-right` is not seen with a hyphen mid-line, join without the hyphen (`unfortunately`, `windows`, `containers`). Otherwise keep the `-` (`billion-dollar`, `first-choice`, `hello-pod`). Default when unsure: **keep the hyphen** (a TTS reads "first-choice" fine, but a wrong join like "firstchoice" is worse). | text fix | H for DDS, M for K8s | DDS, K8s, CA |
| 18 | **Unicode normalize** | NFKC (turns `𝐦𝐞𝐫𝐠𝐞` into `merge` and `ﬁ` into `fi`). Drop U+FE0F, U+200B, and `￾` leftovers. Map `∼` to `~`. Keep smart quotes in the view. Speech may map them to ASCII. | text fix | H | LI (styled letters), K8s |
| 19 | **Speech-only filters** (inside Speak text) | Citations: `\[(\d+([,–-]\d+)*\|[A-Z][A-Za-z+]*\d{2}[a-z]?(,\s*[A-Z][A-Za-z+]*\d{2})*)\]`. URLs: `https?://\S+`, `www\.\S+`, and bare `domain.tld/path` (`informit.com/register`, `inclusivenaming.org`). Emails. Emoji (`\p{Extended_Pictographic}`). Keep the view text as it is. | skip-speech spans | H for `[..]`/URL, M for bare domains | GoF, CA, K8s, LI |
| 20 | **Page cross-refs** (optional) | `\((page )?\d{2,3}\)` after a pattern name (GoF `Visitor (page 366)`). | skip-speech | L | GoF |

Not in v1: splitting GoF joined words (`theWindowImp`), OCR for images (LI infographics), and table structure.

---

## 5. Test fixture list

Format: book, PDF page (label): input feature → expected cleaned output.

**Kubernetes Book**
1. **K8s p12 (label 4):** header `1: Kubernetes primer 4` (y=601.6, 8.0) → dropped; block meta `pdf_page=12, label="4"`. `first￾choice solution` → `first-choice solution`. Headings `Cloud native`, `Microservices` → heading blocks. Bullets `• Web front-end` … → list items "Web front-end", "Catalog", …
2. **K8s p9 (label 1):** heading `0: Preface`. `Unfortu￾nately, …` → `Unfortunately, …`.
3. **K8s p13 (label 5):** `(CNCF)1.` → speech `(CNCF).`. Footnote `1https://www.cncf.io` → ShowOnly footnote, not spoken.
4. **K8s p36 (label 28):** chapter heading `3: Getting Kubernetes` (no header on this page). `free3` → `free`. Footnote 3 → ShowOnly.
5. **K8s p60 (label 52):** YAML block `kind: Pod … <Snip>` → one code block (indent rebuilt: `metadata:` / `  name: …`), speech = cue only. A one-line `$ kubectl logs …` command → its own code block. Heading `kubectl exec`.
6. **K8s p61 (label 53):** terminal output `PID USER TIME COMMAND` → inside the code block. `hello￾pod Pod` → `hello-pod Pod` (must NOT become `hellopod`).
7. **K8s p228 (label 220):** two tables (`apiGroup resource Kubernetes API path`, `Kubernetes verb(s) HTTP method …`) → ShowOnly table + cue. The body line after the first table → spoken.
8. **K8s p5–8 (no label):** TOC → dropped. **p308:** `Index` with no entries → dropped. **p1:** image-only → placeholder.

**Designing Distributed Systems**
9. **DDS p26 (label 12):** footer `12 | Chapter 2: The Sidecar Pattern` → dropped. `internal net‐` + `work security` → `internal network security`. `Many pre-existing applica‐` + `tions` → `applications`. Heading `Dynamic Configuration with Sidecars`. Caption `Figure 2-2. The HTTPS sidecar` → ShowOnly.
10. **DDS p27 (label 13):** footer `Dynamic Configuration with Sidecars | 13` → dropped. `pre￾existing` → `pre-existing`. `notifi‐`+`cation` → `notification`.
11. **DDS p15 (label 1):** `CHAPTER 1` + `Introduction` → one heading "Chapter 1: Introduction". Bottom `1` (y=42.8) → dropped. `object￾oriented` → `object-oriented`.
12. **DDS p18 (label 4):** note box (9.6, x=137) `What is a pattern for a distributed system? …` → Speak, type note, with `dis‐`+`tributed` → `distributed`.
13. **DDS p23 (label 9):** footnote `1 Kubernetes is …` comes *first* in char order → moved out, ShowOnly. Marker in `a pod in Kubernetes,1 but` → speech `a pod in Kubernetes, but`.
14. **DDS p29 (label 15):** `$ docker run … \` (4 lines) and `#!/bin/bash … done` → 2 code blocks. An inline `http://localhost:8080/…` URL → shown, not spoken.
15. **DDS p4 (label ii):** copyright → ShowOnly. **p5 (label iii):** TOC → dropped. **p159 (label 145):** 2-column index → dropped.

**Design Patterns (GoF)**
16. **GoF p11:** header `Design Patterns: Elements of Reusable Object-Oriented Software` → dropped. `11` (13.2 Bold, 2nd in char order) → dropped, label "11" (= pdf page, so it is not shown as a separate label). Must NOT become a heading. Heading `1. Introduction`. Trailing spaces trimmed.
17. **GoF p12:** `…" [AIS+77]. Even …` → speech without `[AIS+77]`. Heading `What is a Design Pattern?`.
18. **GoF p69:** 7.6-size Courier lines → code blocks (font = body font, so the size rule must fire). The body line `stores theWindowImp with which` stays as it is in v1 (known defect).
19. **GoF p21:** Table 1.1 lines (`Purpose`, `Creational Structural Behavioral`, …) + caption `Table 1.1: Design pattern space` → ShowOnly.
20. **GoF p100:** `· a system should …` (one wrapped line) + `and represented.` → one list item, glyph stripped. Heading `Applicability`.
21. **GoF p2:** dotted-leader TOC → dropped. **GoF p416:** Bibliography `[AG90]` entries → ShowOnly. **GoF p15:** the y-gap 684→334 (figure) does not join the paragraphs on either side wrongly.

**Clean Architecture**
22. **CA p21:** marker `1` (size 15, raised) after `put her` → speech `put her …` without the marker. Footnote `1. …` + `programmers.` → ShowOnly. Also: `go to informit.com/register` → the bare domain is not spoken.
23. **CA p20:** `at least 10` + raised `22` → keep (no footnote 22 on the page). View `10²²`. Speech `10 to the 22` or `10^22` (open question). `machines.{xx}` → `machines.`
24. **CA p25:** `billion￾dollar businesses` → `billion-dollar businesses`. Heading `I` + `INTRODUCTION` → one heading, and the outline casing gives "PART I Introduction".
25. **CA p27:** `1` + `WHAT IS DESIGN AND ARCHITECTURE?` → heading "Chapter 1 What Is Design and Architecture?" (from the outline).
26. **CA p28:** block quote (17 italic, x=68.9) `The goal of software architecture is…` → **Speak** (not dropped for being small). Headings `THE GOAL?`, `CASE STUDY`.
27. **CA p53–54:** `Click here to view code image` → dropped. `point.h` / `struct Point;` … → code blocks. The body sentence after the listing → spoken.
28. **CA p29:** `Figure 1.1 Growth of the engineering staff` + `Reproduced with permission…` (italic 17) → ShowOnly caption.
29. **CA p74:** footnote with two URLs → ShowOnly. Marker `publications1 over` → `publications over`.
30. **CA p335:** Index → dropped. **CA p375–429:** image-only "Code Snippets" → placeholder / dropped. **CA p5:** copyright → ShowOnly.

**LinkedIn Posts**
31. **LI p3–7:** TOC → dropped (including the broken p5 entry with leaked body text).
32. **LI p8–9:** heading `Big Endian vs Little Endian` (21.3 Bold) + the paragraph after the image gap. `● Little Endian` → list item "Little Endian". The paragraph continues across the page break p8→p9 as one block.
33. **LI p56:** `When we 𝐦𝐞𝐫𝐠𝐞 𝐜𝐡𝐚𝐧𝐠𝐞𝐬 …` → `When we merge changes …`.
34. **LI p107:** `🔹 What is Sensitive Data?` → list item / subheading "What is Sensitive Data?" with the emoji not spoken. **LI p92:** `▶️ What CSS does?` → the U+FE0F is removed.
35. **LI p1, p368:** image-only → placeholder.

---

## 6. Risks and open questions for the user

1. **Captions:** speak them ("Figure 2-1: The generic sidecar pattern") or skip? I recommend skipping in v1, because the figure itself can't be heard.
2. **Code cue:** is "Code example." OK, or would you rather have silence or a short tone? Should there be a per-book "read code" toggle?
3. **Tables:** skip with a "Table." cue (recommended), or try to read rows? K8s rows collapse into one line, for example `create POST 201 created, 403 Access Denied`.
4. **Exponents** like CA `10^22`: say "10 to the 22"? Rare (1 case found).
5. **GoF missing spaces** (`theWindowImp`, `Forexample`): accept them in v1, or add a careful dictionary split later? A split could break real identifiers like `aFactory`.
6. **Bibliography/copyright:** hide completely, or keep visible but silent? I proposed visible but silent.
7. **LinkedIn infographics** carry most of each post's content, but they have no text layer. OCR would be a separate feature.
8. **CA page labels:** CA has no printed page numbers, so only `pdf_page` exists. The CA index refers to print pages that are not in this PDF.
9. **Font size is nominal** (CA 20 for about 13pt text). All rules must use ratios to the body size. **UNVERIFIED:** whether pdfium-render's `scaled_font_size()` gives the effective size. If it does, use that.
10. **UNVERIFIED:** pdfium-render's char API for the `0x02`/`U+FFFE` hyphen marker and `IsHyphen`. Check this with a small Rust test on K8s p9 before building rule 17.
11. **Risk: header detection on short documents** (< 10 pages) has too few repeats to work. Fall back to "band + digits-only" rules.
12. **Risk: footnote vs. numbered list** (CA `1. Urgent and important` in body vs `1. And she…` footnote). Tell them apart by size (17 vs 20) and position (below the last body line). Fixture 22 plus CA p39 should cover it.
13. **Not verified here:** a true two-column body layout. None of these books has one, so the "fix multi-column order" rule cannot be tested with this set. A two-column paper should be added to the fixtures later.

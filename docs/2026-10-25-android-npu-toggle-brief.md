# Lectrice for Android: NPU toggle options, brief for Fable (2026-10-25)

Status: **options for the user to pick; nothing is built in the app.** Opus wrote this (all research, UX thinking,
options and copy). **Fable builds §3–§8 and makes no design decisions of its own.** Where this brief is silent, keep what
`design/2026-10-23-android-quiet-v2.html` ("the mockup") already does and list it in the progress note.

The feature itself is **blocked on licensing** (`docs/android-logic/22-npu-acceleration.md`, status line). Drawing the
options now lets the user choose while the legal answers are pending; nothing here is a promise that it ships.

Inputs: the logic spec `docs/android-logic/22-npu-acceleration.md` ("file 22": states §4, copy §3.11), the license
review `research/2026-10-25-qnn-license-review.md` §6 (notices, first-use text), the mockup and its briefs
`docs/2026-10-23-android-quiet-v2-brief.md` and `docs/2026-10-24-android-new-screens-brief.md` (style, hooks, screenshot
method), `docs/android-logic/11-settings.md` (T1/T2).

Reading guide for the user: **§1 the three options and my pick, §2 what is the same in all three, §9 questions.** §3
onwards is for Fable.

---

## 1. The options (what changes between them is only *where* the switch lives and how its states get room)

There are two builds (file 22 §2): the **normal edition** (no Qualcomm code) and the opt-in **NPU edition**. The switch
is called **"Use the NPU"**. On supported phones in the NPU edition it is pre-selected and starts right after a one-time
first-use screen (the license needs the user to see its terms). Elsewhere it is shown disabled with a note.

### Option A · "One row" (in the Voice model section of Settings)

- T1's **Voice model** section gets a second row under "Full quality · 370 MB · in use": a switch row "Use the NPU"
  whose **subtitle carries every state** ("Faster speech, less battery", "Getting ready · 312 of 830 MB", "Checking that
  it sounds right…", "Stopped working, so Lectrice uses the CPU"), with a small text button under the subtitle when there
  is something to do ("Try again", "Use Full quality").
- Not available: the row is greyed (title `fg2`, switch disabled) and its subtitle is the note ("Only on certain
  Snapdragon chips" / "Available in the NPU edition on certain Snapdragon chips").
- First-use: a **dialog** over Settings.
- "Remove NPU files" sits on the Voice model page (T2) as a text button under the Full quality card.
- **For:** the smallest change; the switch sits next to the model it belongs to (Full quality only). **Against:** a
  two-line subtitle is tight for reasons and progress; the long first-use text in a dialog needs scrolling at 200 % font;
  no home for the failure reason or the NPU files' size.

### Option B · "Performance section + NPU page" (my pick)

- T1 gets a new section **Performance** between "Voice model" and "Storage". Its row is a **split row**: tapping the
  text opens the **NPU page** (T4n), the switch on the right toggles (the pattern Android's own Wi-Fi row uses). The
  subtitle is a short state ("On · faster speech, less battery").
- Not available: the section header stays and **a note line sits under the header** with an info icon ("Only on certain
  Snapdragon chips" / "Available in the NPU edition on certain Snapdragon chips"), and the row below it is greyed. In the
  normal edition on an allow-listed chip, the note has a text button **About the NPU edition** (opens the sheet NK3).
- **NPU page (T4n):** a status card on top (icon + one state line + one detail line + its action: progress bar and
  "Pause" while downloading, spinner while checking, reason + "Try again" when failed), then the switch row, two plain
  sentences on what it does, the files row ("NPU files · 1.25 GB" + "Remove NPU files"), "Check again", and a link row
  "Qualcomm AI Runtime" (the About entry).
- First-use: a **full page** pushed over Settings (like the first-run screens: title, three short paragraphs, primary
  "Turn on NPU" at the bottom, "Keep using CPU" and "Read Qualcomm's license" under it).
- **For:** the header/note the user asked for falls out naturally; every state has room (progress, reason, size);
  the consent text reads well at 200 % font; the user said "Settings → Voice / Performance". **Against:** one more
  section on T1 and one more page.

### Option C · "Inside the Voice model page"

- T1 doesn't get a new row: the model row's subtitle adds the state ("Full quality · 370 MB · in use · NPU on").
- T2 (Voice model) gets an **NPU card** under the model cards (same card style as the model cards): title "Use the NPU"
  with the switch on the right of the title line, the state line, then its action (progress, "Try again", "Remove NPU
  files"). Not available: the card is greyed with the note as its body.
- First-use: a **bottom sheet** over T2.
- **For:** everything about the voice engine on one page; T1 unchanged. **Against:** two taps to find, and nobody looks
  for a speed switch inside "Voice model"; the greyed card on unsupported phones takes a lot of space for nothing.

**My pick: B.** It is the only option where the "only on certain Snapdragon chips" note is a header the user sees at a
glance (A hides it in a subtitle, C hides it two taps deep), the only one with room for the failure reason and the 1.25 GB
of NPU files (users should see what the space is for), and the consent text gets a real page. A is a fine fallback if
the user wants Settings to stay as short as possible.

## 2. The same in all three options

- **Fallback snackbar (NK1):** over the reader while playing, "The NPU stopped working, so Lectrice switched to the CPU."
  with **Details** (→ Settings, the NPU row/page). Once per failure (file 22 §3.5).
- **About entry (NK2):** Settings → About gets a row "Qualcomm AI Runtime" (NPU edition only), opening a small page with
  the review's text and two rows "View Qualcomm license", "View Qualcomm notices".
- **Edition sheet (NK3):** normal edition, allow-listed chip: "About the NPU edition" sheet with "Open the releases page".
  (In option A it opens from a text button under the row's note; in C from the greyed card.)
- **Storage (NK4):** the Full quality row on the Storage screen reads "370 MB + NPU files 1.25 GB · in use".
- No new colours: greyed = `fg2` text and the disabled switch; states use `okFg`, `errFg`, `warnBg/warnFg`, `accent`.
- Copy: file 22 §3.11 exactly; the extra page/card copy is in §4.4 below.

---

## 3. Build instructions for Fable: the output

- **Edit one file:** `design/2026-10-23-android-quiet-v2.html`. **Add** a new gallery section; keep every existing
  frame, its look, its caption and the live phone unchanged. Allowed changes to existing code: only the
  backwards-compatible hooks of §4.3. After your edits every existing frame must render pixel-identical: re-take
  `android-q2-T1-light`, `android-q2-T1s-light`, `android-q2-T2-light`, `android-q2-T3-light`, `android-q2-R1-light`
  into the scratchpad and compare with `design/screens/`; report the result.
- **The live phone is not touched** (no NPU state in `Live`). Static frames only.
- No new libraries, no network assets (the Google Fonts link stays), no Artifact, no git, nothing downloaded, nothing
  outside `design/` and your progress note `docs/progress/android-npu-options.md`.
- Screenshots: `design/screens/android-npu-<ID>-<theme>[-360].png` (§8). Don't overwrite any existing file.
- All placeholder content is invented (the mockup's books and numbers). **No text from the user's books.**

### 3.1 Page structure

- New HTML right **before** `<section id="questions">`:

```html
<h2 class="group">NPU acceleration · options (25 October)</h2>
<aside class="impact" id="npu-options">…§3.2…</aside>
<h3 class="npu-opt">Option A · One row</h3><div class="gallery" data-group="npuA"></div>
<h3 class="npu-opt">Option B · Performance section + NPU page (recommended)</h3><div class="gallery" data-group="npuB"></div>
<h3 class="npu-opt">Option C · Inside the Voice model page</h3><div class="gallery" data-group="npuC"></div>
<h3 class="npu-opt">The same in every option</h3><div class="gallery" data-group="npuK"></div>
```

- `FRAMES` gets four new keys `npuA`, `npuB`, `npuC`, `npuK` (frames in §5–§7); each frame object gets `isNew: true`
  (the existing `NEW` constant), so captions get the "New" tag.
- `#questions`: append `<h2>Questions (25 October, NPU)</h2>` and an `<ol>` with §9's questions verbatim.

### 3.2 The impact box (page chrome)

`<h3>NPU acceleration: three places for one switch</h3>`, then a `<ul>` with one `<li>` per option: the option's name in
`<strong>` and the first sentence of its first bullet in §1, then "**Recommended: B.**" and the first sentence of §1's
"My pick" paragraph. Then `<h3>Blocked on licensing</h3>` and one `<p>`: "The feature waits for three answers (your
eligibility, Qualcomm in writing, a lawyer). The NPU would ship only in a separate opt-in NPU edition of the app; the
normal edition never contains Qualcomm code."

## 4. Tokens, CSS, icons, data, helpers

### 4.1 Tokens

**No new tokens.**

### 4.2 New CSS (append one block at the end of the phone CSS, before the page-chrome overrides)

```css
/* ===== 2026-10-25: NPU options ===== */
h3.npu-opt{font-size:18px; margin:32px 0 12px}
/* greyed (not available) rows and cards */
.row.dis .t, .row.dis .m, .mcard.dis, .mcard.dis .m{color:var(--fg2)}
.row.dis .sw, .mcard.dis .sw{border-color:var(--line); background:var(--bg2)}
.row.dis .sw::after, .mcard.dis .sw::after{background:var(--line)}
/* note under a section header (option B) */
.secnote{display:flex; gap:8px; align-items:flex-start; padding:0 16px 4px; font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); color:var(--fg2)}
.secnote > svg{width:20px; height:20px; flex:none}
.secnote .tx{flex:1; min-width:0}
.secnote .btn.txt{height:40px; margin:-10px -12px -10px 0; font:500 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans)}
/* split row: text opens the page, switch toggles */
.row.split{padding-right:8px}
.row.split .pick{min-height:48px}
.row.split .vdiv{width:1px; height:32px; background:var(--line); flex:none}
/* a text button under a row's subtitle (option A) */
.row .inl{display:block; margin:4px 0 0 -12px}
.row .inl .btn.txt{height:40px; font:500 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans)}
/* status card on the NPU page and in the NPU card */
.npucard{display:flex; gap:12px; align-items:flex-start; flex-wrap:wrap; border-radius:12px; padding:12px 16px; background:var(--bg2); border:1px solid var(--line)}
.npucard > svg{width:24px; height:24px; flex:none}
.npucard .tx{flex:1; min-width:0}
.npucard .s1{font:500 calc(16*var(--sp))/calc(24*var(--sp)) var(--sans)}
.npucard .s2{font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); color:var(--fg2); margin-top:2px}
.npucard .acts{display:flex; justify-content:flex-end; width:100%; gap:4px}
.npucard.ok > svg{color:var(--okFg)} .npucard.err > svg{color:var(--errFg)} .npucard.busy > svg{color:var(--accent)}
.npucard .pr{width:100%; margin-top:8px}
/* the first-use page's paragraphs */
.fu p{margin:16px 0 0; font:400 calc(16*var(--sp))/calc(24*var(--sp)) var(--sans)}
.fu p + p{color:var(--fg2)}
```

Contrast: only existing pairs (`fg2/bg`, `fg2/bg2`, `okFg/bg2`, `errFg/bg2`, `accent/bg2`). The greyed switch is
non-text and disabled (no contrast requirement); its row title in `fg2` still passes 4.5:1.

### 4.3 Allowed hooks in existing code (backwards-compatible)

| Existing | Change | Existing output stays identical because |
|---|---|---|
| `T1(v, opts)` | add `npu = null`. When set (`{ opt: 'A'|'B', state }`): option A inserts `npuRowA(state)` right after the model row; option B inserts `<div class="sechead">Performance</div>` + `npuSectionB(state)` between the Voice model section and the Storage section; option C replaces the model row's value with `COPY.npu.modelVal[state]` (when defined). The `.scroll` div gets `data-center-cur` when `npu` is set, and the NPU row (or the model row in C) gets the class `cur`, so the existing `positionFrame` centres it | `npu` is null by default |
| `T2(v, opts)` | add `npu = null`. When set: option C appends `npuCardC(state)` after the small-model card; option A appends a text button `Remove NPU files (1.25 GB)` (`btn.txt errc`) under the Full quality card when `state === 'on'` | null by default |
| `T3(v, store, opts)` | if a model in `store.models` has `npuMB`, its meta reads `${m.mb} MB + NPU files ${fmtMB(m.npuMB)} · in use` (the in-use part only when in use) | no existing model has `npuMB` |
| `settingsRow` | none; the new rows are built by the new helpers below | — |
| `T1` About section | when `npu?.edition === 'npu'`, a row "Qualcomm AI Runtime" (chevron) is added after "Open-source licenses" | `npu` null by default |
| `ICONS` | add §4.4 icons | additions only |

`state` is one of: `on`, `onCompile`, `off`, `ready` (download running), `wifi` (waiting for Wi-Fi), `checking`, `failed`,
`needsFull`, `unsupported`, `normal`, `normalAllowed` (normal edition, allow-listed chip). The edition is `normal` for
`normal`/`normalAllowed`, else `npu`.

### 4.4 New icons (Material Icons, 24 × 24; add to `ICONS`)

```js
memory: 'M15 9H9v6h6V9zm-2 4h-2v-2h2v2zm8-2V9h-2V7c0-1.1-.9-2-2-2h-2V3h-2v2h-2V3H9v2H7c-1.1 0-2 .9-2 2v2H3v2h2v2H3v2h2v2c0 1.1.9 2 2 2h2v2h2v-2h2v2h2v-2h2c1.1 0 2-.9 2-2v-2h2v-2h-2v-2h2zm-4 6H7V7h10v10z',
description: 'M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z',
```

If a path renders wrongly, say so and use `info`; don't download anything.

### 4.5 Copy and mock data (add after `COPY`; all invented)

```js
const NPU = { downloadMB: 830, doneMB: 312, filesMB: 1250, pct: 38 };
COPY.npu = {
  title: 'Use the NPU', section: 'Performance',
  sub: {
    on: 'Faster speech, less battery', onCompile: 'On · preparing for long sentences while charging',
    off: 'Off · Lectrice uses the CPU', ready: `Getting ready · ${NPU.doneMB} of ${NPU.downloadMB} MB`,
    wifi: `Waits for Wi-Fi · ${NPU.downloadMB} MB`, checking: 'Checking that it sounds right…',
    failed: 'Stopped working, so Lectrice uses the CPU', needsFull: 'Works with the Full quality voice model',
    unsupported: 'Only on certain Snapdragon chips', normal: 'Available in the NPU edition on certain Snapdragon chips',
  },
  /* option B: the T1 row's short subtitle */
  shortB: { on: 'On · faster speech, less battery', onCompile: 'On · preparing for long sentences', off: 'Off', ready: `Getting ready · ${NPU.pct} %`, wifi: 'Waits for Wi-Fi', checking: 'Checking…', failed: 'Stopped working · using the CPU', needsFull: 'Works with Full quality only' },
  /* option C: the T1 model row's value */
  modelVal: { on: 'Full quality · 370 MB · in use · NPU on', onCompile: 'Full quality · 370 MB · in use · NPU on', checking: 'Full quality · 370 MB · in use · checking the NPU', failed: 'Full quality · 370 MB · in use · NPU stopped', ready: 'Full quality · 370 MB · in use · NPU getting ready' },
  tryAgain: 'Try again', useFull: 'Use Full quality', aboutEdition: 'About the NPU edition', pause: 'Pause',
  mobile: 'Download now on mobile data', remove: `Remove NPU files (${fmtMB(NPU.filesMB)})`, checkAgain: 'Check again',
  /* the NPU page (option B) and card (option C) */
  page: {
    title: 'NPU',
    what: 'Lectrice makes speech on the phone’s Snapdragon NPU instead of its main processor. On the phones we tested it was about twice as fast and used about 40 % less battery. It sounds the same.',
    cpu: 'If the NPU ever stops working, Lectrice switches back to the CPU by itself.',
    files: 'NPU files', filesVal: `${fmtMB(NPU.filesMB)} · part of Full quality`, qnn: 'Qualcomm AI Runtime',
    card: {
      on: ['NPU on', 'Speech is made on the NPU.'],
      onCompile: ['NPU on', 'Preparing for long sentences while charging · 1 of 2'],
      ready: ['Getting ready', `Downloading the NPU files · ${NPU.doneMB} of ${NPU.downloadMB} MB`],
      wifi: ['Waiting for Wi-Fi', `The NPU files are ${NPU.downloadMB} MB. They download on Wi-Fi.`],
      checking: ['Checking that it sounds right…', 'This takes about half a minute.'],
      failed: ['Stopped working', 'The NPU’s speech didn’t match the CPU’s. Lectrice uses the CPU.'],
      needsFull: ['Works with the Full quality voice model', 'The NPU files are made from Full quality.'],
      unsupported: ['Only on certain Snapdragon chips', 'This phone’s chip isn’t one of them.'],
      off: ['NPU off', 'Lectrice uses the CPU.'],
    },
  },
  firstUse: {
    title: 'Use the phone’s NPU?',
    p: [
      'Lectrice can make speech on your phone’s Snapdragon NPU. It’s usually faster and uses less battery. It uses Qualcomm’s closed-source libraries, which are included in this edition under Qualcomm’s license.',
      'By turning it on you agree not to reverse engineer, decompile or disassemble those libraries, and to follow U.S. export and sanctions laws. They’re provided as is, without warranty.',
      'Everything still runs on your phone. Nothing is sent anywhere. You can switch back to the CPU at any time in Settings.',
    ],
    on: 'Turn on NPU', cpu: 'Keep using CPU', license: 'Read Qualcomm’s license',
  },
  snack: 'The NPU stopped working, so Lectrice switched to the CPU.', details: 'Details',
  about: {
    title: 'Qualcomm AI Runtime',
    body: 'This edition of Lectrice includes Qualcomm® AI Runtime (QNN) libraries, © Qualcomm Technologies, Inc. They are proprietary software provided under the Qualcomm AI Stack License, not under Lectrice’s MIT / Apache-2.0 license or the GPL-3.0. They run only in Lectrice’s separate NPU process.',
    license: 'View Qualcomm license', notices: 'View Qualcomm notices',
    tm: 'Qualcomm, Snapdragon and Hexagon are trademarks of Qualcomm Incorporated. Lectrice is not affiliated with or endorsed by Qualcomm.',
  },
  edition: {
    title: 'The NPU edition',
    p: [
      'A separate download of Lectrice that can make speech on the NPU of certain Snapdragon phones, including this one: usually faster and lighter on the battery.',
      'It contains Qualcomm’s closed-source libraries under Qualcomm’s license. Everything else is the same open-source app.',
      'Install it over this app to keep your books, places and settings. Installing this edition again switches back.',
    ],
    open: 'Open the releases page', close: 'Close',
  },
};
```

(`COPY` is a `const` object; adding a key is allowed. Define `NPU` and `COPY.npu` after `fmtMB` exists, or replace the
two `fmtMB(...)` calls with the literal "1.25 GB".)

### 4.6 Helpers (add after `T2`; exact behaviour)

- `const npuOn = (s) => ['on', 'onCompile', 'checking'].includes(s);` — the switch is on for these.
- `const npuDis = (s) => ['unsupported', 'normal', 'normalAllowed', 'needsFull'].includes(s);` — greyed row/card,
  switch disabled (`aria-disabled="true"`, not toggleable).
- `npuSwitch(s)`: `switchCtl(npuOn(s) || s === 'ready' || s === 'wifi', COPY.npu.title, 'npu')`; when `npuDis(s)` add
  `aria-disabled="true"` to the button. While `ready`/`wifi` the switch shows on (the user's choice) and the subtitle says
  why it isn't working yet.
- `npuSubA(s)`: `COPY.npu.sub[s]` (for `normalAllowed` use `sub.normal`).
- `npuRowA(s)`: a `div.row.two` (class `dis` when `npuDis(s)`) with `.tx` = title + `.m` subtitle (`npuSubA(s)`), plus
  `.inl` with one text button: `failed` → "Try again", `needsFull` → "Use Full quality", `normalAllowed` → "About the
  NPU edition", `wifi` → "Download now on mobile data"; `.tr` = `npuSwitch(s)`. Checking: a `spinner()` before the
  switch inside `.tr`.
- `npuSectionB(s)`: if `npuDis(s)` and `s !== 'needsFull'`: first a `.secnote` with `ic('info')` and the note
  (`sub.unsupported` or `sub.normal`), plus, for `normalAllowed`, a text button "About the NPU edition" inside the note.
  Then the **split row** `div.row.split` (class `dis` when `npuDis(s)`): `button.pick` (`data-act="t4n"`) with
  `ic('memory', 'lead')` and `.tx` = title + `.m` (`COPY.npu.shortB[s]`, nothing for the greyed note states, and for
  `needsFull` the `shortB` text), then `<span class="vdiv"></span>`, then `npuSwitch(s)`. Checking: `spinner()` in the
  subtitle line before the text.
- `npuStatusCard(s)`: `div.npucard` + kind (`ok` for on/onCompile, `err` for failed, `busy` for ready/wifi/checking,
  none otherwise) with icon (`check_circle`, `error`, `download` for ready/wifi, `memory` for checking/off/needsFull/
  unsupported), `.s1`/`.s2` from `page.card[s]`, and for: `ready` → `progress(NPU.pct)` and `.acts` "Pause";
  `wifi` → `.acts` "Download now on mobile data"; `checking` → `indeterminate()`; `failed` → `.acts` "Try again";
  `needsFull` → `.acts` "Use Full quality"; `onCompile` → `progress(50)`.
- `T4n(v, s)` (option B's NPU page): `.scroll` → `.tbar` (back + `tl` "NPU") → `px16` `npuStatusCard(s)` → (unless
  `unsupported`) a `settingsRow(COPY.npu.title, '', npuSwitch(s))` (class `dis` for `needsFull`) → `px16` two
  paragraphs `b` (`page.what`) and `bs fg2 mt8` (`page.cpu`) → `sechead` "Files" → `settingsRow(page.files,
  page.filesVal, '')` → for on/onCompile/off/failed: a `row` with `t errc` "Remove NPU files (1.25 GB)" → for on/off/
  failed: `settingsRow(COPY.npu.checkAgain, '', '')` → `sechead` "About" → `settingsRow(page.qnn, '', ic('chevron_right'))`.
  `unsupported`: only the status card and the paragraphs (no switch, no files, no About row).
- `npuCardC(s)`: `div.mcard` (class `dis` when `npuDis(s)`): first line `flex` = `ic('memory')` + `tm sp1` title +
  `npuSwitch(s)`; then `.b mt4` (`page.card[s][1]`, for greyed states the note `sub[s]`), then the action as in
  `npuStatusCard` (progress / Try again / Use Full quality / About the NPU edition), and for `on`: a text button
  `errc` "Remove NPU files (1.25 GB)".
- `firstUseDialog()` (option A): `scrim()` + `dialog(firstUse.title, firstUse.p[0], …)` — but the body holds all three
  paragraphs: build the `.dlg` by hand like `dialog()` with three `div.b` (`mt16` each; the 2nd and 3rd `fg2`) and the
  actions stacked right-aligned in **one column**: `Turn on NPU` (`btn.txt`), `Keep using CPU` (`btn.txt`), `Read
  Qualcomm's license` (`btn.txt`). The dialog's own scroll area (`max-height: calc(100% - 96px); overflow-y:auto`)
  stays scrollable without a scrollbar.
- `firstUsePage()` (option B): a `.layer` with `.col` (padding-top 96): `ic('memory')` in a 48 dp `accentContainer`
  circle, `tl mt24` title, `div.fu` with the three `<p>`; `.bottom` = `btn(on)` + `btn(cpu, 'sec')` +
  `btn(license, 'txt')`.
- `firstUseSheet()` (option C): `scrim()` + `sheet(…, { auto: true })` with `shh` (`tm` title) and `bd` holding the
  three paragraphs (`b`, the 2nd and 3rd `fg2 mt12`) and, at the end, `btn(on)`, `btn(cpu, 'sec', 'style="margin-top:8px"')`,
  `btn(license, 'txt')` centred.
- `aboutQnn(v)` (NK2): `.scroll` → `.tbar` (back + `tl` "Qualcomm AI Runtime") → `px16` `b` body → `sheetRow('description',
  about.license, '', ic('open_in_new'))`, `sheetRow('description', about.notices, '', ic('open_in_new'))` → `px16 bs
  fg2 mt16` trademark line.
- `editionSheet()` (NK3): `scrim()` + `sheet(…, { auto: true })`: `shh` title; `bd` three paragraphs (`b`, then `bs fg2
  mt12` twice); `btn(open, '', '', 'open_in_new')` (mt16) + `btn(close, 'txt')` centred.

## 5. Option A frames (`npuA`)

All on T1 (`T1(v, { npu: { opt: 'A', state } })`, centred on the NPU row) unless stated.

| ID | Name | Note (caption) | Build |
|---|---|---|---|
| NA1 | On | "NPU edition, supported phone. The switch sits under the model it belongs to." | state `on` |
| NA2 | Getting ready | "The NPU files download on Wi-Fi; the CPU is used meanwhile." | `ready` |
| NA3 | Checking | "The self-test after the user agreed (~30 s)." | `checking` |
| NA4 | Stopped working | "After a fallback to the CPU. One tap runs the check again." | `failed` |
| NA5 | Needs Full quality | "Another model is active." | `needsFull` |
| NA6 | Not this chip | "NPU edition on a phone that isn't on the list." | `unsupported` |
| NA7 | Normal edition | "No Qualcomm code; allow-listed chip, so it links to the NPU edition." | `normalAllowed` |
| NA8 | First use (dialog) | "Shown once, before the NPU is used. Turn on is the default choice." | `T1(… 'off')` + `firstUseDialog()` (scrim) |
| NA9 | Voice model page | "Remove NPU files sits under Full quality." | `T2(v, { npu: { opt: 'A', state: 'on' } })` |

## 6. Option B frames (`npuB`)

| ID | Name | Note | Build |
|---|---|---|---|
| NB1 | Settings · on | "A Performance section with a split row: text opens the NPU page, the switch toggles." | `T1(v, { npu: { opt: 'B', state: 'on' } })` |
| NB2 | NPU page · on | "Status, what it does, the files it uses, the license entry." | `T4n(v, 'onCompile')` |
| NB3 | NPU page · getting ready | "Download progress; Pause." | `T4n(v, 'ready')` |
| NB4 | NPU page · waiting for Wi-Fi | "Never on mobile data unless the user asks." | `T4n(v, 'wifi')` |
| NB5 | NPU page · checking | "The self-test runs while the user waits." | `T4n(v, 'checking')` |
| NB6 | NPU page · stopped working | "The reason and Try again." | `T4n(v, 'failed')` |
| NB7 | Settings · not this chip | "The note sits under the section header; the row is greyed." | `T1(… 'unsupported')` |
| NB8 | Settings · normal edition | "Same header note, with a link to the NPU edition on allow-listed chips." | `T1(… 'normalAllowed')` |
| NB9 | Settings · needs Full quality | "The row stays active so the page can offer Use Full quality." | `T1(… 'needsFull')` |
| NB10 | First use (page) | "A full page: the license terms read well at any font size." | `firstUsePage()` over `T1(… 'off')` |
| NB11 | NPU page · not this chip | "Reached from the greyed row only to read why." | `T4n(v, 'unsupported')` |

## 7. Option C frames (`npuC`) and shared frames (`npuK`)

| ID | Name | Note | Build |
|---|---|---|---|
| NC1 | Settings | "No new row; the model row tells the NPU state." | `T1(v, { npu: { opt: 'C', state: 'on' } })` |
| NC2 | Voice model · on | "An NPU card under the model cards." | `T2(v, { npu: { opt: 'C', state: 'on' } })` |
| NC3 | Card · getting ready | | `T2(… 'ready')` |
| NC4 | Card · checking | | `T2(… 'checking')` |
| NC5 | Card · stopped working | | `T2(… 'failed')` |
| NC6 | Card · not this chip | "A greyed card on every unsupported phone." | `T2(… 'unsupported')` |
| NC7 | Card · normal edition | | `T2(… 'normalAllowed')` |
| NC8 | First use (sheet) | | `T2(… 'off')` + `firstUseSheet()` |
| NK1 | Fell back to the CPU | "Once per failure, over whatever is on screen." | `(v) => reader(v) + snackbar…` with action: `<div class="snack" role="status"><span class="sp1">${COPY.npu.snack}</span><button class="act">${COPY.npu.details}</button></div>` (the `.snack .act` CSS exists), extra `RD` |
| NK2 | About · Qualcomm AI Runtime | "NPU edition only; not in the open-source list." | `aboutQnn` |
| NK3 | About the NPU edition | "Normal edition, allow-listed chip." | `T1(v, { npu: { opt: 'B', state: 'normalAllowed' } })` + `editionSheet()` |
| NK4 | Storage | "The NPU files are counted under Full quality." | `T3(v, { ...STORE, models: [{ ...STORE.models[0], npuMB: NPU.filesMB }, STORE.models[1]] })` |

## 8. Checklist and screenshots

- [ ] Only the mockup file edited (plus the progress note and screenshots); existing frames pixel-identical (§3).
- [ ] Every frame in light, dark and black (theme toolbar) and at 360 wide shows no overlap and no visible scrollbar.
- [ ] 200 % font (`x2`): spot-check NA1, NB1, NB2, NB10 (add them as extra frames `NA1x`, `NB1x`, `NB2x`, `NB10x` with
      `{ x2: true }`, caption "200 % font").
- [ ] Greyed rows: `aria-disabled="true"` on the switch; TalkBack-style labels on every icon-only control.
- [ ] Copy exactly as §4.5 (and file 22 §3.11); no new strings.
- [ ] No text from the user's books.
- [ ] Self-review: no unused CSS/JS.

**Screenshots** (device pixel ratio 2, solo mode `?screen=<ID>&theme=<t>&w=<w>`, network blocked, the same headless
Chrome command as `docs/progress/android-quiet-v2.md`; 412 → window 432 × 935, 360 → 380 × 820), new files in
`design/screens/`:

- every frame in light: `android-npu-<ID>-light.png` (NA1–NA9, NB1–NB11, NC1–NC8, NK1–NK4, and the four `x` frames)
- dark: `android-npu-NA1-dark.png`, `android-npu-NB1-dark.png`, `android-npu-NB2-dark.png`, `android-npu-NB7-dark.png`,
  `android-npu-NC2-dark.png`, `android-npu-NK1-dark.png`
- black: `android-npu-NB2-black.png`, `android-npu-NB10-black.png`
- 360: `android-npu-NA1-light-360.png`, `android-npu-NB1-light-360.png`, `android-npu-NB8-light-360.png`,
  `android-npu-NC2-light-360.png`

Progress note `docs/progress/android-npu-options.md`: what was built, the pixel comparison, anything ambiguous you
resolved (most literal reading, listed), the screenshot list, and the line to open the section:
`google-chrome "file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html#theme=light&w=412"`
(then scroll to "NPU acceleration · options").

## 9. Questions for the user

1. **Which option: A (one row), B (Performance section + NPU page, recommended) or C (inside the Voice model page)?**
2. **Normal edition:** show the greyed row with "Available in the NPU edition…" only after an NPU edition exists (my
   default), or never show anything about the NPU in the normal edition?
3. **"About the NPU edition" link** in the normal edition on allow-listed chips (opens the releases page in the
   browser): keep it, or leave only the note?
4. **The first-use screen's buttons:** "Turn on NPU" as the primary button (my reading of "on by default"), OK?

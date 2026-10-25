# 11 · Settings (and the voice model page)

Status: **decided**.

## 1. Goal

One plain screen for everything that isn't changed while listening: defaults for new books, listening behaviour, the two
lock-screen switches, text size and theme, the installed voice models, storage, battery and "about". Every change applies
at once (no Save button).

## 2. User flow

1. Library → ⚙ (top right) → Settings (pushed screen, predictive back to the Library).
2. Rows act in place (switches, segmented controls, slider) or open a small sheet (voice, speed, read ahead) or a pushed
   page (Voice model, Open-source licenses).
3. Voice model → T2: see installed bundles; download the other one; "Use this" to switch; "Delete" the one not in use (with a
   confirm dialog, T2c).
4. Back returns to the Library.

## 3. Rules & logic

### 3.1 Rows (T1, T1b; rows 48 dp; section headers `label` 600 `accent`, padding 24 16 8)

| Section | Row | Control | Values / default | Effect |
|---|---|---|---|---|
| Listening | Default voice / "Heart · US · female" | › opens the voice sheet (file 16) | `af_heart` | voice for books opened from now on (each book remembers its own) |
| | Default speed / "1×" | › opens a sheet with the 6-segment control | 1× | speed for books opened from now on |
| | Read code & tables aloud | switch | off | global; same value as the player sheet's switch |
| | Read ahead / "10 min of listening" | › opens a sheet: 5 / 10 / 20 min | 10 | `set_read_ahead_minutes` (core C14), file 08 |
| | Sentence on the lock screen / "Shows what's being read. Off: the section title." | switch | **on** | media metadata, file 09 §3.3 |
| | Listen view over the lock screen / "Keeps the Listen view up when the screen turns off while it's open." | switch | **off** | file 06 §3.6 |
| Reading | Text size | 5-stop slider S · M · **L** · XL · XXL + a live preview line "The quick reader turns the page." (Noto Serif at that size) | L | file 04 §3.8 |
| | Theme | segmented System · Light · Dark · Black | System | README §4.3; applies to the whole app at once |
| Voice model | "Full quality · 370 MB · in use" | › T2 | — | |
| Storage | **new** "Storage" / "1.2 GB used · 23.4 GB free" | › pushes the **Storage screen** (T3, file 20) | — | models, books (each with "Clear this book's audio" and "Remove from library"), audio cache + "Clear audio cache", exported audiobooks (open, share, save, delete), "Delete all Lectrice data". Replaces T1b's three storage rows (audio cache, library, exported audiobooks) |
| Battery | Keep exports running / "Allow unrestricted battery use" | › opens the system page (file 17) | — | |
| About | "Version 0.1.0 · core 0.2.0" | — | `BuildConfig.VERSION_NAME` + `core_version()` | file 18 |
| | Open-source licenses | › | — | file 18 |
| | Source code on GitHub | › | — | opens the repo URL in the browser |

- The Storage row's numbers: total = `StorageStats` for our package, free = `getAllocatableBytes` (file 20 §3.7). The same
  screen opens from Android's Settings → Apps → Lectrice → Storage → "Clear storage" (`manageSpaceActivity`, file 20 §3.3).
- Every value is stored in DataStore; the ViewModel exposes it as `StateFlow`; the reader and player read the same flows.

### 3.2 Voice model page (T2, T2b, T2c)

- Two cards (S2's cards with state):

| Bundle state | Card line | Buttons |
|---|---|---|
| Installed, in use | "Installed · in use" (`okFg`) | none |
| Installed, not in use | "Installed" | secondary "Use this" · text "Delete (126 MB)" (`errFg`) |
| Not installed | "Not installed" | secondary "Download 126 MB" (runs file 01's download; the card shows its progress) |
| Downloading | progress bar + "142 MB of 370 MB · 38 %" | text "Pause" / "Resume", "Cancel" |

- **Use this:** sets `activeBundle`; if playing, pause → `reload_engine(new bundle)` → play again at the same sentence (status
  "Preparing audio…"); if not playing, reload right away (or lazily on next play, if the engine is unloaded). Cache keys
  differ per bundle (C11), so the new bundle makes its own clips.
- **Delete:** only for the bundle **not in use**. Refused while an unfinished export uses it: sheet **new** "An export uses
  this model" / "Cancel that export first, or wait until it's done." / "OK". Otherwise the confirm dialog (T2c): "Delete the
  smaller model?" / "You can download it again later." / "Cancel" · "Delete" → delete `filesDir/models/<bundleId>/`.
- The in-use bundle can't be deleted (no button). To remove all models: Settings → Storage → "Delete all Lectrice data"
  (Android's "Clear storage" now opens that screen, file 20 §3.3).
- The card notes (speed per phone) follow file 01 §3.8.

### 3.3 Sheets used by rows

- Default voice: the grouped voice list of file 16, with samples; tap = set, sheet closes.
- Default speed: segmented 6 speeds; tap = set, sheet closes.
- Read ahead: three radio rows "5 min of listening", "10 min of listening", "20 min of listening"; tap = set, sheet
  closes.

## 4. States

| State | What shows |
|---|---|
| Loading | rows show at once from DataStore; the Storage row's numbers fill in when ready (≤ 300 ms; "…" before) |
| Error | storage numbers failing → the rows show "—"; logged |
| Offline | "Source code on GitHub" still opens the browser (which shows its own offline page) |
| Download running | T2's card shows progress |
| 200 % font | rows wrap; the text-size slider keeps 48 dp stops |

## 5. Edge cases

- **Theme change while a sheet is open:** the sheet recolours in place.
- **Text size changed while the reader is in the back stack:** re-lays out on return, keeping the current sentence's
  position (file 04).
- **Read ahead changed while playing:** applies from the next sentence (no audio interruption).
- **"Use this" while an export runs on the old bundle:** the export pauses (`EngineSwitch`) and continues when listening
  stops (file 10 §3.6), now waiting for its own bundle.
- **Storage changes while Settings is open** (a book removed on T3, a cache clear): the Storage row recomputes on return.
- **Both bundles deleted by hand (file manager can't; only `adb`):** the app treats it as not installed (L5).

## 6. Where it lives

| Piece | Module |
|---|---|
| `SettingsScreen`, `ModelPage` (T2), row components, `SettingsViewModel`, `ModelPageViewModel` | `feature:settings` |
| `SettingsStore` interface (flows + setters) | `core:model`; DataStore implementation in `core:data` |
| `ModelInstaller` | file 01 |
| Storage screen | file 20 (`StorageScreen` in this module) |
| Core | `storage_usage` (C15), `set_read_ahead_minutes` (C14), `reload_engine`, `core_version`; the Storage screen's calls are listed in file 20 |

## 7. Data and persistence

DataStore (`settings.preferences_pb`), versioned with a `schemaVersion` key:
`defaultVoice`, `defaultSpeedPercent`, `readSkipped`, `readAheadMinutes`, `lockScreenSentence`, `listenOverLockScreen`,
`textSize` (S|M|L|XL|XXL), `theme` (system|light|dark|black), `activeBundle`, plus the hint flags and per-book values of
other files. Backed up by Auto Backup (small); models, library, cache and exports are not.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Rows as in the approved T1 (incl. the two lock-screen switches after "Read ahead") | user (approved v2) | 2026-10-23 |
| Theme System · Light · Dark · Black; System uses dark, not black | design + Opus | 2026-10-23 / 24 |
| Defaults apply to new books; each book remembers its own voice and speed | Opus | 2026-10-24 |
| ~~"Exported audiobooks" storage row + delete~~ → one "Storage" row opening the Storage screen (file 20), which also lists and deletes exports | user (storage visible) + Opus | 2026-10-24 |
| An in-use bundle can't be deleted; a bundle used by an unfinished export can't be deleted | Opus | 2026-10-24 |

## 9. Done when

- [ ] Every row of §3.1 changes its value at once and survives an app restart.
- [ ] Theme Black turns every screen true black (L1-black, R1-black, R13-black match).
- [ ] Text size L → XXL re-lays out the reader keeping the current sentence in view.
- [ ] T2: download the other bundle, "Use this" while playing continues at the same sentence with the new model; delete
      the unused one with the dialog.
- [ ] The Storage row shows the total and free space and opens T3 (its own checks are in file 20 §9).
- [ ] TalkBack reads every row's title, value and control state.

## 10. Design screens

T1, T1b (its storage rows are superseded by the single Storage row), T2, T2b, T2c. **New, needs a design pass (Opus brief
→ Fable):** the Storage row in T1b and the Storage screen T3–T3d (file 20).

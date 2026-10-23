# 20 · Storage (where files live, what Android's Settings shows, the Storage screen)

Status: **decided** (written 2026-10-24 from the user's decisions of that day). Three behaviours are marked UNVERIFIED and
are checked on the S10+ and the S26 Ultra in M4 (§9). Needs core C15, C19, C28, C29.

## 1. Goal

Make every byte Lectrice keeps on the phone **visible and removable**, both in Android's own Settings → Apps → Lectrice →
Storage and in our own **Settings → Storage** screen, and put each kind of file where Android treats it right: things that
can be made again go where Android may clear them; the user's books and finished audiobooks don't.

## 2. User flow

1. **In Android's Settings** → Apps → Lectrice → Storage the user sees "App size", "User data", "Cache" and "Total"
   (AOSP labels; Samsung's wording is UNVERIFIED). "Cache" is our audio cache (plus covers and voice samples).
   - **Clear cache** (system button) deletes it. Nothing else is lost; audio is made again when needed.
   - **Clear storage** (system button) opens **our Storage screen** (T3) instead of wiping everything (§3.3).
2. **In Lectrice:** Settings → **Storage · 1.2 GB** › → the Storage screen (T3):
   - a summary at the top ("1.2 GB used by Lectrice · 23.4 GB free on this phone", a stacked bar),
   - **Voice models** (each installed bundle, › the model page, file 11),
   - **Books** (each book with its size; tap → a sheet: "Clear this book's audio", "Remove from library", file 21),
   - **Audio cache** (bar of the 1 GB cap, "Clear audio cache"),
   - **Exported audiobooks** (each file; tap → Open, Share, Save to Audiobooks, Delete),
   - at the bottom "Free up space on the phone" (the system storage page) and "Delete all Lectrice data".
3. Every delete asks first (except clearing caches), then the numbers update in place.

## 3. Rules & logic

### 3.1 Where each kind of file lives (decided)

| What | Where | Shown in Android's Settings as | Cleared by "Clear cache"? | Android may delete it on low storage? | Why there |
|---|---|---|---|---|---|
| Voice models (`fp32`, `int8`) | `filesDir/models/<bundleId>/` | User data | no | no | big, slow to download again |
| Library (`original.pdf`, `document.json`, `meta.json` incl. the reading position) | `filesDir/lectrice/library/<docId>/` | User data | no | no | the user's books and places |
| **Audio cache** (sentence clips) | **`cacheDir/lectrice/audio/…`** (moved from `filesDir`, core C29) | **Cache** | **yes** | **yes** | made again from the text at any time; the user asked to see and clear it from the system |
| Voice samples | `cacheDir/lectrice/samples/<engineId>/<voiceId>.v1.wav` (C16) | Cache | yes | yes | made again on the next tap |
| Cover tiles | `cacheDir/covers/<docId>.png` (file 02) | Cache | yes | yes | drawn again when missing |
| Import temp | `cacheDir/import/<uuid>.pdf` (file 03) | Cache | yes | yes | deleted after each import anyway |
| Export parts (unfinished exports) | `filesDir/lectrice/exports/parts/<exportId>/` | User data | no | no | hours of work; must survive until the export is done |
| Finished exports (not yet saved) | `filesDir/lectrice/exports/<docId>/<name>.m4b` (file 10 §3.8) | User data | no | no | see §3.4 |
| Exports saved to Audiobooks | shared storage `Audiobooks/Lectrice/` via MediaStore | **not under Lectrice** (counted in the phone's Audio / Files storage; exact category UNVERIFIED on One UI) | no | no | the user's own files |
| Settings | `filesDir/datastore/settings.preferences_pb` | User data | no | no | tiny |

**Facts these rules rest on** (checked in the AOSP sources on 2026-10-24; `developer.android.com` refused the fetch, so the
source files were read instead):

- `Context.getCacheDir()` javadoc: "The system will automatically delete files in this directory as disk space is needed
  elsewhere on the device." and "If your app goes above this quota [`StorageManager.getCacheQuotaBytes`], your cached files
  will be some of the first to be deleted when additional disk space is needed."
- `StorageStats`: `getDataBytes()` includes `getDataDir()`, `getCacheDir()`, `getCodeCacheDir()` (and app-specific external
  dirs); `getCacheBytes()` includes `getCacheDir()` and `getCodeCacheDir()`. The Settings app shows "User data" and "Cache"
  from these (strings `data_size_label` = "User data", `cache_size_label` = "Cache", `clear_cache_btn_text` = "Clear cache",
  `clear_user_data_text` = "Clear storage").
- Settings' **Clear cache** calls `PackageManager.deleteApplicationCacheFiles(package)`; it is enabled only when the cache
  size is > 0 (`AppStorageSettings.java`).
- `StorageStatsManager.queryStatsForPackage(...)`: "no permissions are required when calling this API for your own package."
- `StorageManager.getAllocatableBytes(uuid)`: the most "new bytes that your app can allocate for itself", counting cached
  files "the system may be willing to delete"; `allocateBytes(uuid, n)` "will cause the system to delete any cached files
  necessary to satisfy your request".
- Files in `filesDir`, `cacheDir` and `getExternalFilesDir` are deleted when the app is uninstalled; MediaStore items the app
  inserted into shared folders are not (standard scoped-storage behaviour; Lectrice relies on it only for the
  "survives uninstall" note).

### 3.2 Why the audio cache moves to `cacheDir` (changes file 08's earlier choice)

- **For:** the user wants it visible as "Cache" and clearable from the system; it is fully regenerable; when the phone runs
  low the system frees our cache before other apps fail to save photos; `getAllocatableBytes` then counts it as free space,
  so an export or import isn't refused because of our own cache.
- **Against:** Android may delete clips at any time, so a book may need its audio made again (a few seconds of
  "Preparing audio…", or "Catching up…" on a slow phone). Our 1 GB cap is far above the quota Android gives most apps, so
  under storage pressure our clips go first. Accepted: it only happens when the phone is nearly full, and then freeing
  space is the right thing to do.
- **What keeps it safe:** the core treats a missing, empty or unreadable clip as "not cached" and makes it again (C29); the
  in-memory index re-checks the disk before it reports usage or estimates an export; the clip being played is already in
  ExoPlayer's memory. We **don't** call `StorageManager.setCacheBehaviorTombstone` (it truncates files to 0 bytes instead of
  deleting them) or `setCacheBehaviorGroup`.
- **Export parts stay in `filesDir`**: they are hours of synthesis, not a cache.

### 3.3 "Clear storage" opens our Storage screen (`android:manageSpaceActivity`)

- The manifest's `<application android:manageSpaceActivity=".storage.ManageSpaceActivity">`.
- **What Android does (AOSP `AppStorageSettings.java`, verified):** when an app declares it, the button keeps the label
  **"Clear storage"** (the code sets `clear_user_data_text` either way; older Android versions said "Manage space"), but a
  tap **starts our activity** (`startActivityForResult`) instead of the "Delete app data?" dialog. If the activity can't be
  resolved, the button is disabled. Samsung One UI's label and behaviour: **UNVERIFIED** (checked on both phones in M4).
- So the system button can no longer wipe the library by accident; our screen offers the same power with explanations, and
  "Delete all Lectrice data" at the bottom (§3.6) for a real reset.
- `ManageSpaceActivity` is a thin activity that shows the T3 composable with a close (✕) instead of a back arrow, using the
  app's single `AppContainer` (the Rust core is process-wide). Declared `android:exported="false"` first: the Settings app
  runs with the system uid, which may start a non-exported activity (**UNVERIFIED**). If the M4 test shows it can't, set
  `exported="true"` with no intent filter: it only shows our screen and every delete still needs the user's confirm.

### 3.4 Where exported audiobooks live (recommendation)

**Recommendation: a finished export lives in app storage first, and "Save to Audiobooks" _moves_ it to shared storage.**

| Option | In Android's app Settings | Files app / other players | Uninstall | Double space? |
|---|---|---|---|---|
| A. app-specific only (`filesDir`) | counted in Lectrice's "User data" | not visible (only Share / Save as…) | deleted with the app | no |
| B. shared only (MediaStore `Audiobooks/Lectrice/`, written there directly) | not counted under Lectrice | visible; audiobook apps find it | survives | no |
| C. app storage + "Save" **copies** (the earlier spec) | counted until deleted | visible after save | the saved copy survives | **yes: twice the size** |
| **D. app storage + "Save" moves (chosen)** | counted until saved, then leaves Lectrice's total | visible after save | the saved file survives | no |

- Why D: the export starts private (Android counts it under Lectrice and the user sees it in our screen), the user keeps the
  approved E4 choices (Save to Audiobooks · Save as… · Share), and a 300 MB audiobook never takes 600 MB. Writing straight to
  MediaStore (B) would hide the file from Lectrice's storage numbers, and a cancelled or failed assemble would leave a
  pending item in a shared folder.
- **Move** = MediaStore insert with `IS_PENDING = 1`, streamed copy, `IS_PENDING = 0`, then delete the private file and tell
  the core the new place (`mark_export_saved(id, uri)`, C19). The core record keeps the MediaStore URI, so Lectrice can still
  open, share and delete it (an app may read, change and delete media it inserted, without a permission, while it stays
  installed). If the copy fails, the pending item is deleted and the private file stays.
- Before the move: check `getAllocatableBytes ≥ file size + 50 MB` (the copy needs the room before the private file goes).
- **Save as…** (SAF) copies and keeps the private file (the user may save to a cloud folder).
- **After a reinstall** Lectrice no longer owns the saved files and doesn't list them (it has no media permission); they stay
  in the Files app and audiobook players.

### 3.5 The Storage screen (T3)

Pushed from Settings → **Storage** (the row shows the total, e.g. "1.2 GB used"), or opened by the system's "Clear storage".
Top bar "Storage" (back arrow; ✕ when opened from the system). One scrolling column, Quiet rows (48–72 dp), section headers
as Settings (file 11).

**Summary card** (`bg2`, radius 12, padding 16):

- line 1 `title-m`: "{total} used by Lectrice" where total = `StorageStats.getAppBytes() + getDataBytes()` for our own
  package (`StorageStatsManager.queryStatsForPackage(StorageManager.UUID_DEFAULT, packageName, myUserHandle())`), so it
  matches Android's "Total".
- line 2 `body-s` `fg2`: "{free} free on this phone" = `getAllocatableBytes(getUuidForPath(filesDir))`.
- a 8 dp stacked bar (radius full) with 5 segments in the order of the sections: Voice models, Books, Audio cache, Exports,
  App & other (APK, native libraries, settings: the rest of the total). Colours: the 6 cover hues (file 02 §3.4) in a fixed
  order; a legend below as `caption` chips "● Voice models 370 MB". The bar is decorative for TalkBack (the rows carry the
  numbers).

**Sections:**

| Section | Rows | Tap | Numbers from |
|---|---|---|---|
| Voice models | one per installed bundle: "Full quality" / "370 MB · in use"; a downloading bundle shows its progress | › model page (file 11 §3.2) | directory size of `filesDir/models/<bundleId>/` (Kotlin walk, off the main thread) |
| Books | header row "3 books · 48 MB, plus 212 MB of audio"; then one row per book, **largest total first**: cover tile 40 × 56 · title · meta "PDF and text 16 MB · audio 212 MB" | → book sheet T3b | `storage_usage().documents[]` (C15: `library_bytes`, `cache_bytes` per book) |
| Audio cache | 4 dp bar "312 MB of 1 GB" · text button "Clear audio cache" · note `body-s` `fg2` **new** "Audio is made again when it's needed. Android may also clear it when the phone is low on space." | button: clear at once + snackbar "Cleared 312 MB." | `storage_usage().cache_bytes` |
| Exported audiobooks | one row per export file, newest first: `audio_file` icon · title · meta "Heart · 1.5× · 154 MB · In Lectrice" or "… · In Audiobooks"; an unfinished export: "Exporting · 42 % · 120 MB so far" | → export sheet T3c | `list_all_exports()` (C19: `file_bytes`, `location`, parts bytes) |
| Phone | "Free up space on the phone" › | the system page `StorageManager.ACTION_MANAGE_STORAGE` (+ `EXTRA_REQUESTED_BYTES` when opened from the export's "not enough space") | — |
| | "Delete all Lectrice data" (`errFg`) | dialog T3d (§3.6) | — |

Shown only when not empty: Books (no books → the section shows "No books yet."), Exported audiobooks (hidden when there are
none).

**Book sheet (T3b)** (modal bottom sheet, title = the book title, 2 lines):

- "Clear this book's audio · 212 MB" → no confirm (regenerable) → `clear_document_audio(docId)` (C28) → snackbar **new**
  "Cleared 212 MB of audio." If the book is playing, playback continues (the next sentences are made again).
- "Remove from library" (`errFg`) → the remove flow of file 21 (confirm + undo).

**Export sheet (T3c)** (title = the export's book title; line 2 the summary "Heart · Full quality · 1.5× · 154 MB"):

| Row | Shown when | Action |
|---|---|---|
| Open | done | `ACTION_VIEW`, `audio/mp4`, FileProvider or MediaStore URI with read grant → the system chooser of audio players |
| Share | done | as file 10 §3.9 |
| Save to Audiobooks | done and in Lectrice | the move of §3.4; then the row reads "In Audiobooks" |
| Go to the export | unfinished | opens the reader of that book with its export page (file 10) |
| Delete (`errFg`) | done, failed, cancelled | dialog **new** "Delete this audiobook?" / "This deletes the file from your phone (154 MB)." / "Cancel" · "Delete" → in Lectrice: `delete_export(id)` (C19); in Audiobooks: `ContentResolver.delete(uri)` of our own item, then `delete_export(id)`. A saved file the user already deleted in the Files app: the record is dropped silently. |

An unfinished export can't be deleted here (cancel it on its page, file 10).

### 3.6 Delete all Lectrice data (T3d)

- Dialog **new** "Delete all Lectrice data?" / "This deletes your books, reading places, voice models, audio and exports kept
  in Lectrice. Audiobooks you saved to Audiobooks stay. The app closes." / "Cancel" · "Delete all" (`errFg`).
- Then: stop playback, cancel the workers, `ActivityManager.clearApplicationUserData()` (the same wipe as the old "Clear
  storage"; the process is killed by the system). Next launch starts at S1.

### 3.7 Numbers (how each is computed)

- Directory sizes: sum of file lengths (`File.length()`), not blocks; Android's own numbers count allocated blocks, so ours
  can be a little lower (≤ 2 % expected; UNVERIFIED). The total line uses `StorageStats`, so it matches Android exactly.
- Core numbers (`storage_usage`, C15/C28) re-scan the audio folder before answering (C29), so clips Android deleted are not
  counted.
- Sizes are shown per README §4.7 (MB = 10⁶ bytes, as Android).
- Computed off the main thread on screen open and after every action; rows show "…" until ready (≤ 300 ms target on a
  100-book library; the Kotlin model walk is cached for 10 s).

## 4. States

| State | Shown |
|---|---|
| Loading | rows and the summary card show "…" until the numbers arrive |
| Empty | no books, no exports: "No books yet."; the Voice models section shows "No voice installed" + "Download" (→ S2) |
| Error | a number that fails to load shows "—" (logged); actions still work |
| Offline | no difference |
| Clearing | the button shows a 16 dp spinner; the screen stays usable |
| Opened from the system | ✕ in the top bar; back / ✕ returns to Android's Settings |

## 5. Edge cases

- **Android clears our cache while a book plays:** the current clip is in memory; the next clips are made again
  ("Catching up…" on a slow phone); nothing is shown as an error.
- **The user taps the system "Clear cache" while an export runs:** the export doesn't use cached clips it hasn't read yet
  (it makes them again); its parts are in `filesDir` and stay.
- **"Clear storage" pressed while Lectrice plays in the background:** our screen opens over Settings; playback continues
  (same process).
- **A book is removed while its audio is being cleared:** the remove (file 21) wins; the clear is a no-op.
- **A saved audiobook deleted in the Files app:** the next listing finds the MediaStore URI gone and drops the record
  (logged with the export id).
- **Low storage while moving to Audiobooks:** the pending item is deleted, the private file stays, snackbar with the
  `StorageFull` text.
- **Phone with adoptable/SD storage:** everything lives on internal storage (`UUID_DEFAULT`); Lectrice offers no move.

## 6. Where it lives

| Piece | Module |
|---|---|
| `StorageScreen` (T3), `BookStorageSheet` (T3b), `ExportFileSheet` (T3c), `StorageViewModel` | `feature:settings` |
| `ManageSpaceActivity` (thin host of T3) | `app` (manifest + activity) |
| `StorageInspector` interface (`summary()`, `models()`, `freeBytes()`) | `core:model`; implementation with `StorageStatsManager` / `StorageManager` in `core:data` |
| `ExportFiles` (open, share, move to Audiobooks, delete) | `core:export` (`MediaStoreSaver` grows the move) |
| Core | `storage_usage` (C15 + per-book C28), `clear_audio_cache` (C15), `clear_document_audio`, `remove_document` (C28), `list_all_exports`, `delete_export`, `mark_export_saved` (C19), audio cache in `cacheDir` (C29) |

## 7. Data and persistence

- Paths as §3.1. The core gets two roots from the ffi `CoreConfig`: `data_dir = filesDir/lectrice` and the new
  `cache_dir = cacheDir/lectrice` (C29).
- Export records gain `location` (`AppStorage(path)` | `Shared(uri)`) (C19).
- Auto Backup: unchanged (settings only; file 17 §3.7).
- Nothing new in DataStore.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Cache and exports visible in Android's app Settings; a Storage screen in Lectrice | user | 2026-10-24 |
| Audio cache (and samples, covers) in `cacheDir`, shown as "Cache", cleared by the system's "Clear cache" | Opus, from the user's request (reverses file 08's earlier `filesDir` choice) | 2026-10-24 |
| `manageSpaceActivity` → our Storage screen, with "Delete all Lectrice data" for a real reset | Opus | 2026-10-24 |
| Exports: app storage first, "Save to Audiobooks" **moves** (no double space); "Save as…" copies | Opus (recommendation) | 2026-10-24 |
| Export parts stay in `filesDir` | Opus | 2026-10-24 |

Open: README question 1 (move vs copy on "Save to Audiobooks").

## 9. Done when

- [ ] Android's Settings → Apps → Lectrice → Storage shows the audio cache under "Cache" on both phones; its "Clear cache"
      empties it and Lectrice keeps playing (clips made again).
- [ ] "Clear storage" opens T3 on both phones (record the button's label on One UI in the progress report).
- [ ] T3's total equals Android's "Total" for Lectrice within 1 MB; per-section numbers within 2 % of `du`.
- [ ] Filling the phone (a test file) makes Android delete our clips first; playback recovers; no crash.
- [ ] Save to Audiobooks moves the file: Lectrice's "User data" drops by its size; the Files app and an audiobook app show it.
- [ ] Delete in T3c removes the file in both places (Lectrice and Audiobooks).
- [ ] "Delete all Lectrice data" leaves the app as freshly installed; saved audiobooks remain.
- [ ] No book title or file name in logs (ids and byte counts only).

## 10. Design screens

**New, needs a design pass (Opus writes a brief → Fable draws it, in the Quiet v2 style of
`design/2026-10-23-android-quiet-v2.html` / `docs/2026-10-23-android-quiet-v2-brief.md`):**
T3 (Storage screen, light/dark/black, 360 dp, 200 % font), T3-system (opened from Android's Settings, ✕), T3b (book sheet),
T3c (export file sheet: done in Lectrice / in Audiobooks / unfinished), T3d (delete-all dialog), and the new Settings row
"Storage · 1.2 GB used" that replaces T1b's storage rows.

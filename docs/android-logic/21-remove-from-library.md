# 21 · Remove a book from the library

Status: **decided** (user 2026-10-24: "Remove from library" is in v1; this replaces the earlier "no delete in v1").
Needs core C28.

## 1. Goal

Let the user take a book off the phone in two taps, see what it frees, change their mind for a few seconds, and never lose
an audiobook they exported by accident. Everything the book left behind goes with it: the cleaned text, the PDF copy, its
audio cache and its reading place.

## 2. User flow

1. **Library:** long-press a Recent row or the Continue card → the **book sheet** (L7): the cover tile, title, "PDF and text
   16 MB · audio 212 MB", and the rows "Clear this book's audio · 212 MB" and **"Remove from library"** (`errFg`, `delete`
   icon). TalkBack: custom action "Remove from library" on each row and on the card (no long-press needed).
2. **Settings → Storage → Books:** tap a book → the same sheet (T3b, file 20). This is the visible path for people who
   never long-press.
3. Not offered in the reader or the player sheet (a destructive action doesn't sit next to "Library" while listening; the
   reading-app pattern is to remove from the library list).
4. Tap "Remove from library" → the **confirm dialog** (L7c).
5. Confirm → the dialog and sheet close, the book's row disappears at once (the list animates the gap closed, 200 ms), and
   a **snackbar** (L7u): **new** "Removed “The Long Book”." + **Undo** (8 s, file 12's snackbar rule for an action).
6. Undo → the row comes back in its place; nothing was deleted. No undo → the book is deleted for good (§3.3).

## 3. Rules & logic

### 3.1 The confirm dialog (L7c)

- Title **new** "Remove “The Long Book”?"
- Body **new** "This removes the book, its reading place and its audio from this phone, and frees about {n} MB. You can open
  the PDF again later." where n = library bytes + cache bytes of that book (`storage_usage().documents[]`, C15/C28).
- Extra lines, only when they apply:
  - An unfinished export of this book: **new** "Its export in progress is cancelled."
  - This book is playing: **new** "Playback stops."
- **Exported audiobooks** of this book that live in Lectrice (file 20 §3.4): a checkbox row, **unchecked** by default:
  **new** "Also delete its exported audiobook (231 MB)" (plural "…its 2 exported audiobooks (462 MB)"). Default keep: they
  are the user's finished files. Files saved to Audiobooks are never touched and not mentioned (they're outside Lectrice).
- Buttons "Cancel" · "Remove" (`errFg`).

### 3.2 What happens on confirm (soft remove)

1. If this book is the player's current book: save the position (file 13), **stop** playback (the media session clears its
   item, the notification goes), `stop_read_ahead(docId)` (C13). Undo does **not** restart playback.
2. If it has an unfinished export: `pause_export(id)` (reason `User`, C24), so its worker stops spending battery.
3. Core `remove_document(docId)` (C28): marks the book **removed** (`removedAt` in `meta.json`). From then on `list_recent`
   and the storage numbers skip it; `get_document` returns `DocumentNotFound`; nothing is deleted yet.
4. Kotlin keeps `pendingRemoval = { docId, deleteExports: Bool }` in memory and shows the snackbar.

### 3.3 Purge (the real delete)

Runs when the snackbar times out or is swiped away, when the user removes another book (the first one is purged at once), or
when the app goes to the background (`onStop`), whichever is first. Core `purge_removed_documents(delete_exports_for:
[docId] or [])` (C28) deletes, in this order:

| What | Where | Rule |
|---|---|---|
| Unfinished exports of the book | records + `exports/parts/<exportId>/` | cancelled (parts deleted), records dropped |
| Exported files in Lectrice | `exports/<docId>/*.m4b` + records | **only if the checkbox was ticked**; else kept, and their records keep the title so T3 still lists them |
| Audio cache | `cacheDir/lectrice/audio/<docId>/` (every cleaner version, engine, voice) | `AudioRetention::remove_document` (C28) |
| Library | `library/<docId>/` (`original.pdf`, `document.json`, `meta.json` incl. the position) | the folder, `meta.json` first (so a crash halfway leaves no half-listed book) |

Kotlin then deletes the book's Android-only leftovers: DataStore `voice.<docId>`, `speed.<docId>`, its entry in
`slowTipShown`, any Prepare record (file 14), and `cacheDir/covers/<docId>.png`. If the media-resumption entry (file 09
§3.9) points at this book, it's cleared.

**Process death between confirm and purge:** the book is still marked removed in `meta.json`, so it stays hidden; the core
purges every removed book at the next `open` (startup). Exports are then kept (the checkbox choice was only in memory; the
safe default).

### 3.4 Undo

`restore_document(docId)` (C28) clears `removedAt`. If its export was paused in step 3.2.2, it stays paused (reason `User`)
and the export page shows "Continue". The row comes back at its `lastOpenedAt` place; the Continue card is recomputed.

### 3.5 Opening the same PDF again

- During the undo window: the import finds the id (same SHA-256) **removed but not purged** → the core restores it (no
  re-cleaning) and the snackbar is dismissed without purging.
- After the purge: an ordinary first import (file 03); the reading place is gone; kept exports of that book show again in
  its export page (records are keyed by document id).

## 4. States

| State | Shown |
|---|---|
| Sheet loading sizes | "…" in the meta line until `storage_usage` answers |
| Removing | instant (soft remove); no spinner |
| Purge fails (I/O) | silent for the user (the book is already gone from the list); logged `StorageFailed`; retried at next start |
| Error on soft remove | snackbar with the registry text; the row stays |
| Offline | no difference |

## 5. Edge cases

- **Removing the book shown on the Continue card:** the card moves to the next most recent book with a position (file 02).
- **The last book:** the Library shows L2 (empty) under the snackbar.
- **Removing while the book's audio is being made** (read-ahead or export): the current synthesis finishes; its clip `put`
  is dropped by the core for a removed document (C28).
- **Two removes within 8 s:** the first is purged when the second is confirmed; only the latest has Undo.
- **Opened from Android's "Clear storage" (T3, file 20):** the same flow; the snackbar shows on T3.
- **A cleaner-version re-clean was pending for that book:** irrelevant; it's deleted.

## 6. Where it lives

| Piece | Module |
|---|---|
| Book sheet (L7 / T3b, one composable), confirm dialog, undo snackbar | `core:designsystem` (components), `feature:library` and `feature:settings` (hosts) |
| `RemoveBookUseCase` (soft remove → snackbar → purge, Android leftovers) | `core:data` (behind `LibraryRepository.remove / restore / purge` in `core:model`) |
| Playback stop, read-ahead stop | `PlaybackController` (`core:playback`) |
| Core | **new C28** `remove_document`, `restore_document`, `purge_removed_documents`, `clear_document_audio`; existing `pause_export`, `cancel_export` |

## 7. Data and persistence

- `meta.json` gains `removedAt: Timestamp | null` (C28; serde default null).
- Nothing in DataStore for the undo window (in memory only; §3.3 covers process death).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| "Remove from library" in v1 | user | 2026-10-24 |
| Offered by long-press (Library), TalkBack action, and Settings → Storage; not in the reader | Opus | 2026-10-24 |
| Deletes the cleaned text, PDF copy, audio cache and reading place; exported files **kept by default** (checkbox to delete) | user (default keep) + Opus | 2026-10-24 |
| Soft remove + 8 s undo snackbar, purge after | Opus | 2026-10-24 |
| Playing → stops; unfinished export → paused, cancelled at purge | Opus | 2026-10-24 |

No open questions.

## 9. Done when

- [ ] Long-press a row → sheet → Remove → confirm: the row goes, the snackbar shows; Undo brings it back with its place.
- [ ] Without Undo: `library/<docId>/` and its audio folder are gone; T3's numbers drop by the shown amount (± 1 %).
- [ ] Exports are kept unless the checkbox is ticked; saved-to-Audiobooks files are never touched.
- [ ] Removing the playing book stops playback and clears the notification; removing a book mid-export cancels it at purge.
- [ ] Killing the app inside the undo window: the book stays hidden and is purged at the next start.
- [ ] TalkBack: "Remove from library" is reachable as a custom action on every row.
- [ ] No title in logs (the document id hash only).

## 10. Design screens

**New, needs a design pass (Opus brief → Fable, Quiet v2 style):** L7 (book sheet from a long-press), L7c (confirm dialog,
with and without the export checkbox and the "playback stops" line), L7u (undo snackbar over the Library). T3b is the same
sheet drawn in file 20.

# 17 · Permissions and system integration

Status: **decided** (two behaviours are UNVERIFIED and checked on the phones in M3: the media-notification exemption and
the Now Bar).

## 1. Goal

Ask for as little as possible, at the moment it makes sense, explain it in one sentence first, and keep working when the
user says no. Fit into Android's background rules (foreground services, the 6-hour limit, Samsung's battery management),
the file system (no storage permission), "Open with", and backup.

## 2. User flow

1. The first time a background job or playback is about to start (model download, export, or the first play, whichever
   comes first) on Android 13+: our small sheet (R10) explains why → "Continue" → the system dialog (R10b) → Allow / Don't
   allow. Either way the action goes on.
2. If exports get stopped early (E3), "Samsung phones may stop it sooner: allow unrestricted battery" → the system's battery
   page for Lectrice.
3. Nothing else is ever asked: no storage, no microphone, no location, no contacts.

## 3. Rules & logic

### 3.1 Permissions in the manifest

| Permission | Why | Asked at runtime? |
|---|---|---|
| `POST_NOTIFICATIONS` (API 33+) | download, export, prepare progress; the media notification | yes, once (§3.2) |
| `FOREGROUND_SERVICE` | playback, download, export | no (normal) |
| `FOREGROUND_SERVICE_MEDIA_PLAYBACK` | `PlaybackService` | no |
| `FOREGROUND_SERVICE_DATA_SYNC` | `ModelDownloadWorker` (all APIs), `ExportWorker` below API 35 | no |
| `FOREGROUND_SERVICE_MEDIA_PROCESSING` (API 35+) | `ExportWorker`, `PrepareWorker` | no |
| `WAKE_LOCK` | ExoPlayer's local wake mode while playing | no |
| `INTERNET`, `ACCESS_NETWORK_STATE` | model download, metered check | no |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | **not used** (we only open the settings page, no direct request dialog) | — |

No `READ_EXTERNAL_STORAGE` / `READ_MEDIA_*`: files come through the system picker (SAF); audiobooks are moved to shared
storage through MediaStore inserts of our own files, and later opened, shared or deleted by Lectrice as items it owns (no
permission needed on API 29+; file 20 §3.4). `StorageStatsManager.queryStatsForPackage` for our own package needs no
permission either (verified in the AOSP source, file 20 §3.1).

### 3.2 Notification permission (R10, R10b)

- Asked **once ever** (DataStore `notificationAsked`), on Android 13+, right before the first of: starting a model download,
  starting an export, the first play.
- Our sheet first (R10), copy by trigger:
  - first play: "Show the player outside the app?" / "Lectrice shows the player in your notifications and on the lock
    screen, so you can pause it without opening the app." / "Continue";
  - download or export: **new** "Show progress outside the app?" / "Lectrice shows the download and export progress in your
    notifications, and the player on the lock screen." / "Continue".
- Then the system dialog. **Don't allow** → nothing breaks: playback, downloads and exports work; progress is visible only
  in the app.
- Media session notifications are documented as **exempt** from this permission on API 33+ (so the lock-screen player may
  still show when denied): UNVERIFIED on One UI; checked in M3 and written in the progress report. If it holds, the "first
  play" trigger can be dropped later (the download nearly always comes first anyway).
- We never ask again. Settings has no notification row; Android's app settings handle it.

### 3.3 Notification channels

| Channel | Importance | Used by |
|---|---|---|
| "Playback" | low (no sound) | the media notification (Media3 creates it) |
| "Downloads" | low | N4 progress |
| "Exports" | low | N3 progress, waiting, paused |
| "Results" | default | "The voice is ready", "Audiobook ready", failures |

### 3.4 Background work and limits

- Playback: foreground `mediaPlayback` service while playing (exempt from Doze while it plays).
- Download: WorkManager foreground `dataSync`.
- Export / Prepare: WorkManager foreground `mediaProcessing` (API 35+) / `dataSync`. Android 15+ allows **6 h per 24 h** for
  these types; `onTimeout` → pause, then regular work while charging (file 10 §3.7). The S10+ (Android 12) has no such limit.
- **Battery page:** Settings → Battery → "Keep exports running" and E3's text button open
  `Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS` (the list; Samsung shows "Unrestricted" under the app's battery
  page). A one-line explanation is on the row; no nagging, no automatic prompt.

### 3.5 Over the lock screen

`Activity.setShowWhenLocked(true)` only while the Listen view is open and its setting is on (file 06 §3.6). No permission.
`requestDismissKeyguard` for actions that need unlocking. No `USE_FULL_SCREEN_INTENT`.

### 3.5b Storage in Android's Settings (file 20)

- `<application android:manageSpaceActivity=".storage.ManageSpaceActivity">`: Android's Settings → Apps → Lectrice → Storage
  → "Clear storage" opens our Storage screen instead of wiping the app (AOSP behaviour verified; One UI UNVERIFIED; the
  activity's `exported` value per file 20 §3.3).
- The audio cache, voice samples, covers and import temp live in `cacheDir`, so Android shows them as "Cache" and its
  "Clear cache" button clears them; the system may also clear them on low storage (file 20 §3.2).
- Battery and heat signals read by the read-ahead governor (file 08 §3.3): `PowerManager.isPowerSaveMode` +
  `ACTION_POWER_SAVE_MODE_CHANGED`, `PowerManager.getCurrentThermalStatus` + `addThermalStatusListener` (API 29+), sticky
  `ACTION_BATTERY_CHANGED`, `ACTION_BATTERY_LOW` / `ACTION_BATTERY_OKAY` (registered at runtime, not in the manifest). No
  permission.

### 3.6 Intents and links

| Intent | Handled by |
|---|---|
| `ACTION_VIEW`, `application/pdf` ("Open with Lectrice") | `MainActivity` → import (file 03) |
| `ACTION_SEND`, `application/pdf` ("Share to Lectrice") | same |
| Media button / resumption | `PlaybackService` (file 09) |
| Outgoing: Share an export | `ACTION_SEND` with a `FileProvider` URI or our MediaStore URI (file 10) |
| Outgoing: Open an export | `ACTION_VIEW`, `audio/mp4`, read grant (file 20 §3.5) |
| Outgoing: "Open phone storage" | `StorageManager.ACTION_MANAGE_STORAGE` + `EXTRA_REQUESTED_BYTES` (file 10 §3.11, file 20) |
| Incoming: "Clear storage" in Android's app Settings | `ManageSpaceActivity` (file 20 §3.3) |
| Outgoing: "Source code on GitHub", licenses links | the browser (`ACTION_VIEW` https) |

`MainActivity` is `singleTask`; new intents arrive in `onNewIntent`.

### 3.7 Backup and data extraction

- `dataExtractionRules` / `fullBackupContent`: include **only** the DataStore settings file. Exclude `filesDir/models/`,
  `filesDir/lectrice/` (library, exports, parts) and `cacheDir` (audio cache, samples, covers, import temp; Auto Backup
  skips `cacheDir` anyway).
- Reason: the library can be hundreds of MB (Android's cloud backup quota is 25 MB per app), the model is downloadable, and
  the cache is regenerable. A restored phone keeps the settings and needs the model download again; books must be opened
  again (positions are lost with the library; accepted for v1).

### 3.8 Edge-to-edge and system bars

All screens are edge-to-edge (enforced from targetSdk 35) and pad by the insets; the status bar icons are dark on light
themes and light on dark/black themes; the gesture bar is transparent.

## 4. States

| State | Shown |
|---|---|
| Permission denied | no change in the app; no in-app nag |
| Battery restricted | exports pause more often (E3) |
| Locked over the Listen view | N2L marks (file 06) |

## 5. Edge cases

- **The user revokes notifications later:** progress is still visible in the app; playback still works.
- **"Don't keep activities" developer option:** the navigator restores from saved state (file 12 §3.6).
- **Work profile / second user:** separate app data; nothing special.
- **Restored from backup on a new phone:** settings only; S1 shows again (no model installed), with the user's theme.

## 6. Where it lives

`app` (manifest, `MainActivity`, `ManageSpaceActivity`, permission flow, intents, the backup rules XML in
`app/src/main/res/xml/`), `core:playback` (media service, read-ahead governor's system signals), `core:export` (workers,
channels). No core change for this file (the storage ones are in file 20).

## 7. Data and persistence

DataStore: `notificationAsked`. Backup rules as §3.7.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Ask at the first of download / export / first play, not only at first play | Opus (changes spec §7.1) | 2026-10-24 |
| No storage permission (SAF + MediaStore) | spec §9 | 2026-10-23 |
| Battery page link, no direct exemption request | spec §8.3 | 2026-10-23 |
| Back up settings only | Opus (spec §6.3 already excluded model and cache) | 2026-10-24 |
| `manageSpaceActivity` → our Storage screen; caches in `cacheDir` | user (storage visible in Settings) + Opus | 2026-10-24 |

## 9. Done when

- [ ] A fresh install asks for notifications exactly once, with the right sheet for the trigger.
- [ ] With notifications denied, play, download and export all work; the lock-screen player behaviour is recorded.
- [ ] `aapt dump permissions` lists exactly §3.1.
- [ ] A backup/restore test (`adb shell bmgr`) restores settings only.
- [ ] Android's app Storage page: "Clear storage" opens our Storage screen; "Clear cache" clears the audio cache (file 20 §9).

## 10. Design screens

R10, R10b, E3 (battery link), T1b (battery row), N3, N4. T3 (Storage screen, also the `manageSpaceActivity` target): new,
needs a design pass (file 20).

# 18 · About, licenses and app updates

Status: **spike** (whether the APK carries GPL-3 code depends on the sherpa-onnx 2.0 decision; the rest is decided).

## 1. Goal

Say honestly what the app is made of (versions, open-source licenses, source code), and let users update by installing a
newer APK from GitHub Releases without losing their books, places or model.

## 2. User flow

1. Settings → About: "Version 1.0.0 · core 1.1.0"; "Open-source licenses" ›; "Source code on GitHub" ›.
2. Open-source licenses → a list of components with their license; tap one → its full text.
3. Updating: the user downloads the new APK from the repo's Releases page and installs it over the old one (Android asks
   to allow installs from the browser/file manager the first time). Everything stays.

## 3. Rules & logic

### 3.1 Version line

`"Version {BuildConfig.VERSION_NAME} · core {core_version()}"`. `core_version()` = the core's `[workspace.package]` version
(the tag it was built from). Long-press copies the line to the clipboard with **new** snackbar "Copied." (handy for bug
reports; the only long-press in Settings).

### 3.2 Licenses screen

- A hand-kept list (no license-plugin dependency) + the texts in `assets/licenses/` (spec §14.1):

| Component | License |
|---|---|
| Lectrice (Kotlin + Rust) | MIT OR Apache-2.0 |
| Kokoro 82M model and voices (downloaded) | Apache-2.0 (its NOTICE is in the bundle) |
| sherpa-onnx | Apache-2.0 |
| espeak-ng (inside sherpa-onnx 1.13.x) + espeak-ng-data | **GPL-3.0** — only while the app uses sherpa-onnx 1.x |
| onnxruntime | MIT |
| pdfium (+ freetype, icu, libjpeg-turbo, …) | BSD-3-Clause and others (its `licenses/*.txt`) |
| Jetpack Compose, AndroidX, Media3, WorkManager, DataStore, Kotlin, coroutines | Apache-2.0 |
| JNA | Apache-2.0 (dual-licensed with LGPL-2.1; used under Apache-2.0) |
| uniffi runtime | MPL-2.0 (UNVERIFIED in the spec's pass; check before release) |
| Sonic time-stretch (port, core C10) | Apache-2.0 |
| Kokoro lexicon files | UNVERIFIED (check source and license before publishing, spec §14.1) |

- **If the APK uses sherpa-onnx 1.13.x**, the screen starts with a short notice (**new**): "This app includes espeak-ng,
  licensed under the GNU GPL version 3. You can get the complete source code at github.com/kia-salvati/lectrice-android and
  github.com/kia-salvati/lectrice, at the tag of this version." The release page carries the same notice and links (spec
  §14.3). On sherpa-onnx 2.0 the notice and the GPL rows go.

### 3.3 Updates (GitHub Releases only)

- Releases: signed APKs built by GitHub Actions (spec §14.3); the signing key is made once, backed up, stored as Actions
  secrets. **The same key forever**, or users can't update over their install.
- `versionCode` increases with every release; `versionName` = semver of the Android app; the release notes name the core
  tag ("Core: v1.1.0").
- **No in-app update check in v1** (proposed later: a "new version available" check against the GitHub Releases API, which
  would be the app's only other network use).
- **What survives an update:** everything in `filesDir` (library, positions, cache, exports, models) and DataStore.
  - A higher **cleaner version** in the new core → each book is re-cleaned the next time it opens ("Updating this book's
    text…", file 03).
  - A new **engine id** for the same bundle (never planned) would only mean new cache clips.
  - DataStore has a `schemaVersion`; each release that changes keys ships a migration (read old key → write new → delete old)
    run at startup before any screen.
  - Export records and parts load across core versions (serde defaults, file 19 §5).
- **Model updates:** the model manifest is compiled into the app. A release that ships a different bundle (new id) shows the
  new bundle on the model page as "Not installed" next to the old one, which keeps working; no forced download. (No such
  release is planned.)

### 3.4 "Source code on GitHub"

Opens `https://github.com/kia-salvati/lectrice-android` in the browser.

## 4. States

| State | Shown |
|---|---|
| Offline | the browser shows its own offline page for the GitHub link |
| Missing license text (build error) | caught by a unit test (§9); never shown |

## 5. Edge cases

- **Downgrade** (an older APK over a newer one): Android refuses (`versionCode`); the user would have to uninstall (data lost).
  Release notes say so.
- **Installing a debug build over a release build:** different keys → refused; expected.
- **Core version mismatch** between the bindings and the `.so`: impossible in a release (built together); a debug check at
  startup compares `core_version()` with the version the bindings were generated for and throws.

## 6. Where it lives

`feature:settings` (`AboutRows`, `LicensesScreen`), `app` (build config, version), `assets/licenses/`, GitHub Actions
workflow in the Android repo. Core: `core_version()` (ffi, spec §5.2).

## 7. Data and persistence

DataStore `schemaVersion`. Nothing else.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| GitHub Releases only; signed APK via Actions | user | 2026-10-23 |
| No in-app update check in v1 | spec §14.3 (proposed later) | 2026-10-23 |
| GPL notice at the top of the licenses screen when on sherpa 1.x | Opus (new copy), per spec §14.3 | 2026-10-24 |

Open (spike): sherpa-onnx 2.0 (no GPL) or 1.13.8 (GPL-3 APK); the uniffi and lexicon license checks.

## 9. Done when

- [ ] A unit test checks that every component in the list has a text file in `assets/licenses/`.
- [ ] Installing release N+1 over N keeps books, places, cache, exports and the model; a cleaner-version bump re-cleans on
      open.
- [ ] The version line shows both versions; long-press copies it.

## 10. Design screens

T1b (About rows). The licenses screen is a plain list (not drawn; uses the standard row and a text page).

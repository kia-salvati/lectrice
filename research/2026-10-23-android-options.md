# Lectrice for Android: how to build it (research, 2026-10-23)

Scope: pick how to build the Android app for Lectrice. The app reuses the Rust core (`backend/crates/lectrice-{domain,infra,app}`)
through a new `lectrice-ffi` crate. This is research only: no app code was written and no SDK was installed.

Method:
- **Telegram:** the real repo `github.com/DrKLO/Telegram`, branch `master` (last push 2026-10-20, app version 13.0.3).
  I read the file tree through the GitHub API (14,688 files) and downloaded and read the key files.
- **sherpa-onnx v1.13.8 Android archive:** downloaded and inspected with `readelf`/`nm`/`strings`.
- **pdfium-binaries `chromium/7881` android-arm64:** downloaded and inspected the same way.
- **Source code read:** sherpa-onnx's Kotlin API and the `sherpa-onnx-sys` 1.13.8 `build.rs` from the local cargo registry.
- **Versions:** crates.io and the GitHub release APIs.
- **Web:** published phone benchmarks and Android docs.

Anything I could not check is marked **UNVERIFIED**.

---

## 0. The short answer

- **Build it in native Kotlin with Jetpack Compose. Copy Telegram's ideas, not its framework.** The pieces:
  - one Activity;
  - our own small design system, built from design D's tokens;
  - custom drawing only where it pays (the reader text, if it is ever needed);
  - spring animations;
  - strict "no work on the main thread";
  - few dependencies.
- **Use the platform for the hard background parts.** Media3 (`MediaSessionService` + ExoPlayer) handles playback, the lock screen and
  Bluetooth buttons. WorkManager handles export.
- **Rust core for everything that is "the product":** PDF → clean document → sentences → TTS → cache → read-ahead plan → export
  plan. It is reached through `uniffi`. Kotlin owns only Android things: UI, the media session, files (SAF), the background
  scheduler and audio encoding with `MediaCodec`.
- **Telegram verdict:** it is possible, but it is the wrong trade for this project.
  - Telegram is about 1.3 million lines of Java. A small expert team has built it since 2013.
  - It has its own navigation stack, its own fork of RecyclerView, a 10,000-line theme engine and hand-drawn chat cells. It needs
    all of that because it renders huge, media-heavy chat lists on very old phones.
  - Lectrice has two screens. Its hard problems are on-device TTS speed and background audio, not list performance.
- **Biggest risk: Kokoro may be too slow on a phone CPU.** Published numbers put it near real time or slower (§3.4). Milestone 1
  is a go/no-go spike that measures it on the user's own phone before any UI work starts.

---

## 1. How Telegram for Android is actually built

All paths are in `github.com/DrKLO/Telegram` (`master`, 2026-10-20). The sizes and line counts come from the file tree and from
the files I downloaded.

### 1.1 Language, size, build

| Item | What the source says | Where |
|---|---|---|
| Language | **Java.** Main sources hold 2,207 `.java` files (≈ 72 MB) and **0 Kotlin files**. The 703 `.kt` files are only in tests (`TMessagesProj_AppTests`) and Gradle plugins (`buildSrc`). | tree |
| Java level | `sourceCompatibility JavaVersion.VERSION_1_8`, with core-library desugaring | `TMessagesProj/build.gradle` |
| SDK | `minSdkVersion 23`, `targetSdkVersion 36`, `compileSdkVersion 36`, NDK `27.2.12479018`, CMake `3.22.1`, `-DANDROID_STL=c++_static` | `TMessagesProj/build.gradle` |
| Build | Gradle (Groovy DSL), AGP `8.13.2`, R8 full mode, multidex; the Kotlin plugin is used for buildSrc only | `build.gradle`, `gradle.properties` |
| Modules | `TMessagesProj` (a library holding everything) plus thin app shells: `TMessagesProj_App`, `_AppHuawei`, `_AppStandalone`, `_AppHockeyApp`, `_AppTests` | `settings.gradle` |
| Biggest files | `ui/ChatActivity.java` **47,485 lines** (2.6 MB), `ui/Cells/ChatMessageCell.java` **29,584 lines**, `ui/LaunchActivity.java` 9,250, `ui/ActionBar/Theme.java` 10,159, `tgnet/TLRPC.java` 2.9 MB | downloaded |
| License | **GPL-2.0 "or later"** (file headers: *"It is licensed under GNU GPL v. 2 or later"*) | `LICENSE`, `ui/ActionBar/BaseFragment.java` header |

### 1.2 UI approach: their own framework on top of plain Android Views

- **One Activity, with fake "fragments" that are really Views.** `ui/LaunchActivity.java` is the main Activity
  (`actionBarLayout = new ActionBarLayout(this, true)`, line 458). Every screen extends
  `ui/ActionBar/BaseFragment.java` (1,490 lines), which is **not** an Android Fragment. It is a plain class with
  `createView(Context)`, `onFragmentCreate()`, `onFragmentDestroy()`, `presentFragment(...)`, `finishFragment()` and
  transition hooks (`onTransitionAnimationStart/Progress/End`, lines 745–756).
- **Navigation stack:** `ui/ActionBar/ActionBarLayout.java` (3,601 lines, `extends FrameLayout implements INavigationLayout`).
  - It keeps the screen stack itself and animates pushes and pops.
  - It implements **swipe-back by hand** in `onTouchEvent` (lines 1412–1460). The swipe starts after 0.4 cm horizontally
    (`AndroidUtilities.getPixelsInCM(0.4f, true)`) when dx/3 > dy, and then moves `containerView.setTranslationX(...)` directly.
  - While a transition runs, it switches the screen to a **hardware layer** (`setLayerType(LAYER_TYPE_HARDWARE)`, line 1404), so
    the GPU moves one cached bitmap instead of redrawing.
  - The manifest declares only 19 `<activity>` entries, mostly widgets, share and calls.
- **Lists:** `ui/Components/RecyclerListView.java` (3,914 lines) extends RecyclerView. But Telegram **excludes the official
  `androidx.recyclerview` artifact** (`exclude group: 'androidx.recyclerview'` in `TMessagesProj/build.gradle`) and ships **its own
  fork**: 39 files in `TMessagesProj/src/main/java/androidx/recyclerview/widget/`, with `RecyclerView.java` at 599 KB.
- **Layouts are written in code, not XML.** There are only 22 files under `res/layout`. Layout uses helpers such as
  `ui/Components/LayoutHelper.createFrame(width, height, gravity, l, t, r, b)` in dp, and `AndroidUtilities.dp(...)`.
  `ui/Components/` holds 792 files of custom widgets.
- **Custom drawing:** a chat bubble is not a tree of TextViews. `ChatMessageCell extends BaseCell` and draws text itself with
  `StaticLayout` objects (127 uses in that file) inside `onDraw`. That means one View per message, with measure and draw done by hand.
- **Own theme engine:** `ui/ActionBar/Theme.java` has **826 colour keys** (`public static final int key_... = colorsCount++`),
  302 shared static `Paint`/`TextPaint`/`Drawable` objects, theme files, accents and wallpapers. Each screen lists its themed
  parts in `getThemeDescriptions()` (`BaseFragment`, line 895; `ThemeDescription.java`), so a theme can change live without
  recreating screens.
- **Own animation helpers:** `ui/Components/CubicBezierInterpolator.java`, `AnimatedFloat.java`, plus AndroidX
  `dynamicanimation` (springs).
- **Own event bus and threads:** `messenger/NotificationCenter.java` uses int event ids (`didReceiveNewMessages = totalEvents++`).
  `messenger/DispatchQueue.java` runs background loopers.

### 1.3 How much AndroidX/Jetpack it uses

Very little, and only for small utility pieces (from `TMessagesProj/build.gradle`):

- **Used:** `androidx.core`, `palette`, `exifinterface`, `dynamicanimation`, `sharetarget`, `interpolator`, `biometric`, `webkit`,
  `mediarouter`, `car.app`, `credentials`.
- **Not used:** Compose, ViewModel, Navigation, Room or WorkManager.
- **Media:** a **vendored fork of Media3** (submodule `TMessagesProj_Modules/media` → `github.com/Arseny271/media`), used as
  `media-lib-exoplayer`, `-session`, `-decoder-ffmpeg`, and so on.
- **The audio player service still uses older APIs.** `messenger/MusicPlayerService.java` uses `MediaSessionCompat` (line 115)
  and the even older `RemoteControlClient` (line 259) with `startForeground(...)`.

### 1.4 Native code (JNI, C/C++)

- `TMessagesProj/jni/CMakeLists.txt` builds one big library (target `tmessages.49`).
- **Their own code:** `jni/tgnet/` (the MTProto network stack in C++, wrapped by `TgNetWrapper.cpp`; the Java side is
  `tgnet/ConnectionsManager.java`), `voip/` (4,387 files, WebRTC), image, gif and lottie helpers.
- **Third-party code, as git submodules (`.gitmodules`):** ffmpeg, libvpx, dav1d, opus/ogg/opusfile, libyuv, openh264,
  boringssl, abseil, **tdlib** (`jni/td`), tgcalls, rlottie (`tlottie`), wamr, boost-regex and sqlite.

### 1.5 Why it feels fast and smooth

1. **A flat view tree.** One custom-drawn View per list row means measure, layout and draw stay cheap, and no inflation happens
   while scrolling.
2. **No XML inflation and no reflection** when building screens. All screens are built in code.
3. **Transitions it controls.** Screens are Views in one window, so a push is a translation of two hardware-layered containers
   that follows your finger.
4. **Strict threading.** Network, database and decoding run on their own `DispatchQueue` threads and in C++. The UI thread only
   draws.
5. **Shared, pre-built paints and drawables** (`Theme.java`), so drawing creates no new objects and causes no garbage-collection
   pauses.
6. **Years of tuning by people who know the View system very deeply.**

### 1.6 What it costs

| Cost | Detail |
|---|---|
| Code size | ≈ 1.3 M lines of Java in main sources (72 MB ÷ ~55 bytes per line, measured on `ChatActivity.java`). The UI framework alone (navigation, theme, RecyclerView fork, layout helpers, bottom sheets) is tens of thousands of lines. |
| Maintenance | Every Android platform change must be handled by hand in their own framework. Examples: predictive back gestures (on by default for apps targeting Android 16 / API 36; UNVERIFIED in detail here), edge-to-edge (enforced from targetSdk 35), insets, accessibility and foldables. Jetpack libraries get these fixes for free. |
| Learning curve | You must master the classic View system before you write the first screen: `onMeasure`/`onLayout`/`onDraw`, `Canvas`, `StaticLayout`, touch dispatch, `ValueAnimator` and hardware layers. Very little current tutorial material teaches this style, and the code style (47 k-line classes, static globals) is not something to copy into a clean-architecture project. |
| License | Copying any Telegram code would make Lectrice GPL. The core is Apache-2.0 today (`backend/Cargo.toml`). Note that the APK will already include GPL-3 code through espeak-ng (§3.5). |
| Fit | Telegram needs the custom framework for thousands of mixed media rows on minSdk-23 phones. Lectrice needs a library list, a reader and a player. |

**Verdict:** the Telegram *engineering principles* are worth copying (§4.1). The Telegram *framework* is not worth rebuilding
for this app or for one developer who is learning frontend.

---

## 2. Options compared

### 2.1 The options

- **a. Telegram-style:** Kotlin/Java, custom Views, our own navigation and theme, minimal Jetpack.
- **b. Kotlin + Jetpack Compose** (Material 3 as a base, with our own design tokens), Media3 and WorkManager.
- **c. Kotlin + classic Views + RecyclerView** (XML or code layouts, Fragments, Navigation).
- **d1. Flutter** (Dart) + `flutter_rust_bridge`.
- **d2. React Native** + `uniffi-bindgen-react-native`.
- **d3. Kotlin Multiplatform + Compose Multiplatform** (shared UI with iOS) + Gobley/uniffi.
- **d4. Tauri 2 mobile:** the existing React frontend in a WebView, with the Rust core in the same process.

### 2.2 Big comparison

| | a. Telegram-style | **b. Compose** | c. Classic Views | d1. Flutter | d2. React Native | d3. KMP + CMP | d4. Tauri 2 |
|---|---|---|---|---|---|---|---|
| Smoothness ceiling | Highest, if you are an expert | High (release builds + baseline profiles); custom `Canvas`/`TextMeasurer` drawing is available when needed | High | High (Impeller renderer) | Medium–high | High on Android; on iOS it is not native UIKit | Medium: WebView scroll and gesture feel, no native transitions |
| Effort to reach "awesome" | Very high | Medium | Medium–high | Medium | Medium | Medium–high | Low for a first version, high for polish |
| Rust core plugs in via | uniffi Kotlin (JNA) or hand-written JNI | **uniffi Kotlin** (JNA); the same path Mozilla uses for Firefox's Rust components (`mozilla/application-services`) | uniffi Kotlin | `flutter_rust_bridge` (2.14.0-beta.2, 2026-09-12; latest stable UNVERIFIED) | `uniffi-bindgen-react-native` (0.31.0-6, 2026-09-25; tracks uniffi; maturity UNVERIFIED) | Gobley (v0.3.7, **last release 2025-10-08**, so it may lag uniffi 0.32) or hand-written cinterop | Direct Rust calls in-process (Tauri commands). `sherpa-onnx-sys` already copies its `.so` files into Tauri's `gen/android/app/src/main/jniLibs` (`build.rs` lines 70–73) |
| Background audio (media session, lock screen, Bluetooth) | Media3 by hand (Telegram uses `MediaSessionCompat`) | **Media3 `MediaSessionService` + ExoPlayer**: first class | Media3 | `audio_service` + `just_audio` plugins (wrap ExoPlayer on Android); the Rust feeding path is more complex | `react-native-track-player` (wraps Media3; UNVERIFIED version) | Media3 in `androidMain` | A **custom Kotlin plugin is needed anyway**. WebView `<audio>` has no media session; without one, newer Android may silence background audio (UNVERIFIED) |
| Long work (read-ahead, export) | Service / WorkManager | **WorkManager + foreground service**, `CoroutineWorker` | WorkManager | Platform channels to WorkManager | Native module | WorkManager in `androidMain` | Custom plugin |
| File access (SAF picker) | `ACTION_OPEN_DOCUMENT` | `rememberLauncherForActivityResult(OpenDocument())` | Same intent | `file_picker` plugin | Library | `androidMain` | Tauri dialog plugin (UNVERIFIED for content URIs) |
| APK size overhead (UI only) | Smallest | ≈ 2–5 MB after R8 (UNVERIFIED) | Small | Engine ≈ 5–8 MB per ABI (UNVERIFIED) | Hermes + RN ≈ 7–10 MB (UNVERIFIED) | Like Compose on Android | WebView is the system's, so small; plus the bundled web assets |
| Reuse of web frontend | Design tokens and copy | **Design tokens, copy, logic ideas** (state machine of `usePlayback`, `playerStatus`, `progress` utils, ported by hand) | Same as b | Tokens only | **TS types/utils** and React know-how; not DOM, CSS or `<audio>` code | Same as b | **Most of `frontend/src`** |
| iOS later | No | Rust core via uniffi Swift; UI rewritten in SwiftUI, or move to CMP later | No | Yes (shared) | Yes (shared) | **Yes (shared UI)** | Yes |
| Learning curve for this user | Steepest | **Gentle: Compose feels like React** (§2.3) | Steep and outdated | New language (Dart) | Easiest start (React) | Compose + KMP Gradle + iOS toolchain | Easiest start |
| Long-term maintenance | Heaviest | **Lowest: Google's default path; most docs and samples** | Medium (legacy direction) | Plugin and engine upgrades | RN upgrades are known to be painful | Young tooling (Gobley) | Two worlds (web + Kotlin plugins) |

### 2.3 Why Compose suits a React learner

| React (what you know) | Compose |
|---|---|
| Function component | `@Composable fun` |
| Re-render | Recomposition (only the parts whose inputs changed) |
| `useState` / `useMemo` | `remember { mutableStateOf(...) }` / `remember(key) { ... }` |
| `useEffect` | `LaunchedEffect(key) { ... }` (a coroutine) |
| Props down, events up | The same rule ("state hoisting") |
| A hook with fetch + state | A `ViewModel` exposing a `StateFlow`, collected with `collectAsStateWithLifecycle()` |
| CSS variables (`tokens.css`) | A `CompositionLocal` theme object (`LectriceTheme.colors.hl`) |

Kotlin itself will feel close to Rust: null safety, `sealed` classes ≈ enums with data, `data class` ≈ `#[derive(Clone, PartialEq)]`
structs, and coroutines ≈ async.

### 2.4 Per-option verdicts

- **a. Telegram-style:** no, for the reasons in §1.6.
- **b. Compose:** **recommended.** Its quality ceiling is enough for a reader app. It is the official direction, and Media3 and
  WorkManager are designed around it.
- **c. Classic Views:** no. You would get Telegram's learning curve without Telegram's payoff, on a toolkit Google no longer
  pushes for new apps.
- **d1. Flutter:** a good tool, but it adds Dart, and background audio fed by Rust is harder than in plain Kotlin. Choose it only
  if iOS becomes a must on day one.
- **d2. React Native:** it reuses React skills, but the binding tooling is young, and lock-screen playback goes through
  third-party wrappers of the same Media3 anyway.
- **d3. KMP/CMP:** a reasonable *later* step. Write the Android app in Compose with clean module borders, then move the UI to
  CMP when iOS is real. The Rust-for-KMP tooling (Gobley) is the weak link today.
- **d4. Tauri 2:** the fastest way to *a* working app, possibly even running the existing axum server in-process. But it needs
  the same Kotlin media plugin, and it cannot reach the "clean and awesome" feel. It is OK as a throwaway experiment, not as the
  product.

---

## 3. Our native stack on Android

### 3.1 sherpa-onnx (TTS)

**Verified from the `sherpa-onnx-v1.13.8-android.tar.bz2` release asset (43 MB):**

| ABI | `libsherpa-onnx-c-api.so` | `libonnxruntime.so` | `libsherpa-onnx-jni.so` | `libsherpa-onnx-cxx-api.so` |
|---|---|---|---|---|
| arm64-v8a | 4.5 MB | 22.2 MB | 4.8 MB | 0.4 MB |
| armeabi-v7a | 3.2 MB | 15.4 MB | 3.4 MB | 0.3 MB |
| x86_64 | 4.9 MB | 25.6 MB | 5.2 MB | 0.4 MB |
| x86 | 5.1 MB | 26.5 MB | 5.4 MB | 0.4 MB |

- **What each library is for** (the bundled README says so): the **Rust/C route** needs `libsherpa-onnx-c-api.so` +
  `libonnxruntime.so`. The **Kotlin/Java route** needs `libsherpa-onnx-jni.so` + `libonnxruntime.so`. The `c-api` library
  depends only on `libonnxruntime.so`, `libandroid.so`, `liblog.so`, `libm`, `libdl` and `libc` (`readelf -d`).
- **16 KB page size: OK.** All arm64 `.so` files have LOAD alignment `0x4000`. Google Play requires 16 KB page support for apps
  targeting Android 15+.
- **The Rust crate on Android:** `sherpa-onnx-sys` 1.13.8 `build.rs` forces **shared** linking on Android/iOS (lines 104–106).
  It downloads the Android archive and picks `jniLibs/<abi>/` (lines 171–180). Gradle must then package
  `libsherpa-onnx-c-api.so` + `libonnxruntime.so` next to our own `liblectrice_ffi.so`. To control which files get packaged,
  point `SHERPA_ONNX_LIB_DIR` at a vendored `jniLibs/arm64-v8a`.
- **A smaller variant exists:** `sherpa-onnx-v1.13.8-android-static-link-onnxruntime.tar.bz2` (33 MB for all ABIs) folds
  onnxruntime into one `.so`. Whether the Rust build script accepts it is UNVERIFIED.
- **Kotlin API:** `sherpa-onnx/kotlin-api/Tts.kt` provides `OfflineTts(assetManager, config)` with
  `generate`/`generateWithCallback`/`generateWithConfig` and an `OfflineTtsKokoroModelConfig`. Official AARs exist
  (`sherpa-onnx-1.13.8.aar`, 47 MB). Sample apps: `android/SherpaOnnxTts` and `android/SherpaOnnxTtsEngine` (a Compose UI that
  registers as a system TTS engine).
- **New today: sherpa-onnx v2.0.0** (GitHub release 2026-10-23). The Rust crate on crates.io is still 1.13.8. On `master`,
  Kokoro's `dataDir` is marked *"deprecated, ignored; use lexicon or phonemeCodepoints"*, and a comment says sherpa-onnx *"does
  not use espeak-ng anymore"*. If that holds for Kokoro, it could remove both the 19 MB `espeak-ng-data` and the GPL question
  (§3.5). UNVERIFIED for Kokoro; check when 2.x reaches crates.io.

### 3.2 pdfium

**Verified from `pdfium-android-arm64.tgz` at `chromium/7881`** (the tag that matches `pdfium-render` 0.9.4):

- One `lib/libpdfium.so` of **6.3 MB**. It depends only on `libc`, `libm` and `libdl`, and is **16 KB aligned** (`0x4000`).
- The release also ships arm, x64 and x86 builds, plus `-v8` variants that include a JavaScript engine (not needed).
- The latest tag is `chromium/8111` (2026-10-19). Stay on 7881 to match the bindings.
- `pdfium-render` binds at runtime with `libloading`. Its README says Android's *system* pdfium is not usable by apps, so we
  ship our own.
- **Gotcha (UNVERIFIED):** with modern AGP packaging, native libs are **not extracted** from the APK, so
  `nativeLibraryDir/libpdfium.so` may not exist as a file. Bind by **bare name** (`Pdfium::bind_to_library("libpdfium.so")`, so
  the Android linker finds it inside the APK), or set `packaging.jniLibs.useLegacyPackaging = true`. Test this in the spike.

### 3.3 Model files: ship them or download them?

| Bundle (sherpa `tts-models` release) | Download size | Notes |
|---|---|---|
| `kokoro-multi-lang-v1_0` (what we use) | 333 MB tar.bz2. Locally: `model.onnx` 311 MB, `voices.bin` 27 MB, lexicons 5.7 + 6.1 MB, `espeak-ng-data` 19 MB, plus zh files we don't need | fp32 |
| `kokoro-int8-multi-lang-v1_0` | 126 MB | int8, **may be slower on CPU** (below) |
| `kokoro-int8-en-v0_19` / `kokoro-en-v0_19` | 98 / 304 MB | older, English only |
| `kitten-nano-en-v0_8-int8` / `-fp32` | 29 / 60 MB | much faster, lower quality |

- Google Play limits apply to the compressed download. The base module is listed at **200 MB**, while asset packs can be larger
  (up to 1.5 GB each, 4 GB in total). Snapshots of the page disagree, so check the live page:
  https://support.google.com/googleplay/android-developer/answer/9859372.
- **Recommendation: download on first run, keep the APK lean (~40–45 MB for arm64).**
  - Ship arm64-v8a only at first.
  - Fetch the model from a pinned GitHub release URL and check its SHA-256.
  - Unpack it into `filesDir/models/` (sherpa needs real file paths on the Rust/C route; the Kotlin `AssetManager` route is not
    available through the C API).
  - Also offer "import model from file" for fully offline installs.
  - The app stays fully local after the one-time download.
  - Drop the zh files.
- **Rough APK estimate for arm64 (UNVERIFIED):** sherpa 26.7 MB + pdfium 6.3 MB + Rust core 3–8 MB + JNA ≈ 1 MB + Compose/Media3
  dex ≈ 4 MB.

### 3.4 Speed and memory on phones (published numbers; no Kokoro-on-sherpa number exists for a modern mid-range phone)

| Source | Device | Model | Result (RTF = compute time ÷ audio time; < 1 is faster than real time) |
|---|---|---|---|
| Voiceping offline TTS study (2026-02-15), https://voiceping.net/en/blog/research-offline-tts-eval/ | Samsung Galaxy S10 (Exynos 9820), 4 threads, sherpa-onnx | Kokoro en v0.19 (fp32) | **RTF 1.133** (slower than real time) |
| same | same | Kokoro **int8** multi-lang v1.1 | **RTF 2.423** |
| same | same | Kitten Nano fp16 / Piper low | RTF 0.387 / 0.077 |
| same (iOS) | iPhone | Kokoro EN v0.19 | **833 MB memory** |
| muxable/rtchat PR #1514, https://github.com/muxable/rtchat/issues/1514 | Ryzen 7950X3D, 2 threads / Android x86_64 emulator | Kokoro v1.1 fp32 / int8 | 0.37 / 0.96 desktop, 0.59 / 1.52 emulator. *"int8 was 2.5–3.5× slower than fp32 on x86"* |
| sherpa-onnx discussion #3968, https://github.com/k2-fsa/sherpa-onnx/discussions/3968 | 2-core Neoverse-N1 | Kokoro (Python onnxruntime) | ≈ 1.15 at 2 threads. More threads than cores made it **worse** |
| Speech Central, 2026-10-17, https://speechcentral.net/2026/10/17/kokoro-for-android-from-proof-of-concept-to-practical-offline-tts/ | Motorola Razr 2020, Adreno 620 GPU (OpenCL, not sherpa) | Kokoro | ≈ 0.93 warm streaming, *"close to real-time"* |
| Our laptop (spec §4) | Ryzen 9 6900HX, 8 threads | Kokoro v1.0 fp32 | 0.23 (4.4× real time) |

What this means for the app:

- **At 1× speed a mid-range phone may only just keep up. At 2× it probably can't.**
- int8 is not an automatic win on ARM; measure both.
- Execution providers exist in sherpa (`csrc/provider.cc`: `xnnpack`, `nnapi`), but whether Kokoro runs faster on them on
  Android is UNVERIFIED. NNAPI is deprecated by Google from Android 15.
- Sustained synthesis will heat the phone and drain the battery (thermal throttling UNVERIFIED).
- **Design answers already supported by our architecture:**
  1. **"Prepare" mode:** generate whole chapters, or the whole book, in the background while charging. The sentence cache
     already makes this resumable, and listening then just plays from the cache.
  2. **Another `SpeechEngine` adapter** (Kitten, Piper, or the new "7M Kokoro distills" from the v2.0.0 notes, UNVERIFIED) as a
     "fast voice".
  3. **Later:** let the desktop app pre-render and transfer the audio.
  4. **The spike decides** (§4.4).

### 3.5 Licenses to keep in mind

- espeak-ng is **GPL-3.0** (`espeak-ng/espeak-ng` license API), and it **is linked into** `libsherpa-onnx-c-api.so` and
  `libsherpa-onnx-jni.so`: 49 and 48 `espeak` strings respectively, plus `phontab` (checked with `strings`).
- So a distributed APK must meet GPL-3 terms. Our Apache-2.0 code is compatible inside a GPL-3 whole, and the repo will be
  public anyway. Not legal advice; confirm before publishing.
- pdfium is BSD-3. Its bundled `licenses/*.txt` (freetype, icu, libjpeg-turbo, …) must be shown in an "Open-source licenses"
  screen.

### 3.6 Rust core for everything, or sherpa's Kotlin API for TTS?

**Use the Rust core for everything.**

| | Rust core for all (recommended) | Kotlin sherpa API for TTS + Rust for PDF |
|---|---|---|
| Single source of truth | Yes. `SpeechEngine` port, sentence cache, read-ahead plan (scaled by speed), export plan and voice table are already written and tested in Rust | Duplicated in Kotlin, or split across the FFI boundary |
| Native libs | One onnxruntime + c-api (≈ 27 MB) | onnxruntime + jni (+ c-api if Rust still links sherpa) |
| Swappable engine rule (CLAUDE.md) | Kept: nothing outside the adapter knows Kokoro | Broken: Kotlin knows sherpa configs |
| iOS later | The same Rust core | Rewrite TTS in Swift too |
| Risk | Android build of `sherpa-onnx-sys` untested end to end | The Kotlin AAR is the officially tested path |

**Fallback:** if the Rust Android build of sherpa fails in the spike, write a `KotlinSherpaSpeechEngine`. It is a uniffi *foreign
trait* implemented in Kotlin with the official AAR, plugged into the Rust core as its `SpeechEngine`. The architecture stays the
same; only the adapter moves.

---

## 4. Recommendation

### 4.1 "Telegram principles, Compose tools"

What we copy from Telegram, and how it looks in Compose:

| Telegram principle | In Lectrice |
|---|---|
| One Activity, screens it controls | One `MainActivity`; Navigation 3 or a tiny back stack of our own state (two or three screens) with shared-element and spring transitions |
| Own theme engine | `LectriceTheme`: a small token object ported from `frontend/src/styles/tokens.css` (`bg`, `bg2`, `fg`, `fg2`, `line`, `accent`, `hl`, …), light and dark, serif reading type. Material 3 only as the base for widgets |
| Custom drawing where it matters | The reader paragraph is a `Text` with an `AnnotatedString` (highlight span). If profiling shows jank on huge blocks, switch that one composable to `Canvas` + `TextMeasurer` ("custom View only for the reader", inside Compose) |
| Nothing heavy on the UI thread | All core calls go on `Dispatchers.IO`; Rust owns its own speech worker thread |
| Few dependencies | Compose, Material 3, Lifecycle/ViewModel, Media3, WorkManager, DataStore, JNA (for uniffi), coroutines. No DI framework (a manual `AppContainer` = the composition root, like `lectrice-api`) |
| Tuned animations | `spring()` everywhere, gesture-driven sheets (chapters panel, player popover), `animateItem` in lists |
| Measured, not guessed | Baseline profile + Macrobenchmark/JankStats on a 400-page book |

### 4.2 App architecture

The Gradle modules mirror the backend layers, with dependencies pointing inward:

```
android/                                 (new, separate Gradle project, like backend/ and frontend/)
  app/                 MainActivity, AppContainer (composition root), navigation
  core/model/          pure Kotlin domain models + interfaces the UI needs:
                       LibraryRepository, DocumentRepository, PlaybackController, ExportScheduler, ModelInstaller
  core/data/           the ONLY module that sees uniffi-generated code (`uniffi.lectrice.*`):
                       RustCore wrapper, repository impls, record mappers, error mapping, SAF import, model download
  core/playback/       LectricePlaybackService (MediaSessionService), ExoPlayer, SentenceDataSource, read-ahead trigger
  core/export/         ExportWorker (CoroutineWorker + foreground), MediaCodec/MediaMuxer encoder (Kotlin side of the exporter port)
  core/designsystem/   LectriceTheme tokens, MiniPlayer pill, popover sheet, page-marker chip, icons
  feature/library/     LibraryScreen + ViewModel (open PDF, recent documents with progress, continue)
  feature/reader/      ReaderScreen + ViewModel (reading view, page markers, chapter label, chapters panel, player UI)
  rust/                Gradle task: cargo-ndk (4.1.2) → jniLibs/arm64-v8a; uniffi-bindgen → Kotlin sources
```

- **Data flow:** `RustCore` → repositories (`core/data`) → ViewModel `StateFlow` → screens → components. Events flow back up.
  This is the same rule as the web frontend's `api → hooks → page → components`.
- **Fakes:** `core/model` interfaces get fakes for previews and tests, just like `frontend/src/api/mock`.

**Playback design (the key Android piece):**

- Each spoken sentence is one Media3 `MediaItem` with the URI `lectrice://<docId>/<sentenceId>?voice=<id>`.
- A custom `DataSource` resolves it by calling `core.speakSentence(...)` (blocking, on ExoPlayer's loader thread). That call
  returns the **path of the cached WAV**, so nothing is copied across the boundary.
- ExoPlayer's own buffering preloads the next items, and Rust read-ahead keeps the queue about 10 listening-minutes ahead.
- **Speed:** `player.setPlaybackSpeed(rate)`. Pitch is preserved, and audio stays at 1×, so the cache key does not change
  (the same decision as the web).
- **Highlight:** `Player.Listener.onMediaItemTransition` → `mediaId` = sentence id → the reader's `StateFlow`.
- **For free from Media3:** lock screen, notification, Bluetooth and headset buttons, audio focus and "becoming noisy".
- **Manifest:** `foregroundServiceType="mediaPlayback"` + `FOREGROUND_SERVICE_MEDIA_PLAYBACK`
  (https://developer.android.com/media/media3/session/background-playback).

**Export design:**

- WorkManager `CoroutineWorker` with `setForeground` (type `dataSync`). Rust runs the plan and synthesis; Kotlin encodes.
- **Limit:** on Android 15+, `dataSync`/`mediaProcessing` foreground services get **6 hours per 24 h**. After that
  `onTimeout()` fires and the service must stop. The timer resets when the user opens the app
  (https://developer.android.com/develop/background-work/services/fg-service-timeout).
- An 8-hour book at RTF ≈ 1 needs about 8 h, so **export must be resumable.** It already is: sentences are cached, so
  re-running continues. Add a "while charging" constraint.
- **Formats:**
  - **M4B/M4A:** AAC through `MediaCodec` + `MediaMuxer`. MediaMuxer can't write chapter atoms, so chapters need a small
    MP4 post-process (in Rust or Kotlin; UNVERIFIED).
  - **MP3:** Android has no MP3 encoder, so it would need LAME (LGPL, dynamic). Suggest **M4B only on mobile at first.**
  - **Output:** `ACTION_CREATE_DOCUMENT`, or MediaStore in `Audiobooks/`.

**Files:**

- Import uses `ActivityResultContracts.OpenDocument(arrayOf("application/pdf"))`. Kotlin copies the content URI into
  `cacheDir` and passes the path to Rust. `ImportPdf` already copies the original into the library, so there is no need to keep
  persistable URI permissions.
- Library and cache live under `filesDir/` (private; no storage permission needed).
- The model lives under `filesDir/models/`.

**minSdk:** suggest **29** (Android 10: scoped storage, `DIRECTORY_AUDIOBOOKS`). Phones fast enough for Kokoro are newer than
that anyway. **targetSdk:** the current one (36).

### 4.3 The Rust ↔ Kotlin boundary (`lectrice-ffi`)

- `backend/crates/lectrice-ffi`: `crate-type = ["cdylib"]`, `uniffi` 0.32.2 proc-macros.
- It is a **composition root**, the same role as `lectrice-api`: it builds the infra adapters and app use cases from a config
  that Kotlin passes in.
- Tip: move the shared wiring out of `lectrice-api` into a small function or crate both can call, so the two roots don't drift.

```rust
#[derive(uniffi::Record)] pub struct CoreConfig { data_dir: String, model_dir: String, pdfium_library: String,
                                                  num_threads: u32, read_ahead_minutes: u32, keep_minutes_per_document: u32, cache_max_mb: u32 }

#[derive(uniffi::Object)] pub struct LectriceCore { /* Arc<services> */ }

#[uniffi::export] impl LectriceCore {
    #[uniffi::constructor] fn new(config: CoreConfig, exporter: Arc<dyn AudioEncoderSink>, log: Arc<dyn LogSink>) -> Result<Arc<Self>, CoreError>;
    fn health(&self) -> EngineHealth;                                         // ok | failed (model missing)
    fn import_pdf(&self, pdf_path: String) -> Result<ImportResult, CoreError>;   // summary + already_in_library
    fn list_recent(&self, page: u32, page_size: u32) -> Result<SummaryPage, CoreError>;
    fn get_document(&self, id: String) -> Result<Document, CoreError>;           // blocks, sentences, page labels
    fn get_toc(&self, id: String) -> Result<Vec<TocEntry>, CoreError>;           // chapters panel
    fn save_position(&self, id: String, sentence_id: String) -> Result<(), CoreError>;
    fn list_voices(&self) -> VoiceList;
    fn speak_sentence(&self, id: String, sentence_id: String, voice: String, rate: f32) -> Result<String, CoreError>; // cached WAV path; queues read-ahead
    fn cancel_read_ahead(&self, id: String);
    fn start_export(&self, id: String, voice: String, format: ExportFormat) -> Result<String, CoreError>;  // export id
    fn run_export_step(&self, export_id: String, budget_ms: u64) -> Result<ExportStatus, CoreError>;      // worker calls in a loop; resumable
    fn export_status(&self, export_id: String) -> Result<ExportStatus, CoreError>;
    fn cancel_export(&self, export_id: String) -> Result<(), CoreError>;
}

#[uniffi::export(with_foreign)] pub trait AudioEncoderSink: Send + Sync {      // Kotlin implements with MediaCodec
    fn begin(&self, target_path: String, sample_rate: u32) -> Result<u64, CoreError>;
    fn start_chapter(&self, handle: u64, title: String) -> Result<(), CoreError>;
    fn append(&self, handle: u64, pcm16: Vec<i16>) -> Result<(), CoreError>;
    fn finish(&self, handle: u64) -> Result<String, CoreError>;
}
#[uniffi::export(with_foreign)] pub trait LogSink: Send + Sync { fn log(&self, level: LogLevel, target: String, fields: String); }

#[derive(uniffi::Error)] pub enum CoreError { App { name: String, code: String, message: String } }  // from the error registry
```

Boundary rules:

- **Coarse-grained calls.** uniffi's Kotlin side uses JNA (`net.java.dev.jna:jna` ≥ 5.12.0, `@aar`; latest 5.19.1), which costs
  microseconds per call. That is fine per sentence, but never per word or per audio sample.
- **Blocking functions, with Kotlin choosing the thread** (`withContext(Dispatchers.IO)`). Avoid uniffi async/tokio at first:
  fewer moving parts. The uniffi docs advise attaching long-lived Rust threads to the JVM if they call back into Kotlin.
- **Errors** carry the registry `name`/`code`/`message`, never book text. Logging goes through `LogSink` → Logcat with ids and
  counts only (the existing logging rule).
- **`get_document` for a 400-page book is a few MB of records.** If mapping it is slow, page it by block ranges (UNVERIFIED;
  measure in M2).
- **New core work this needs** (small, all in adapters):
  - an encoder adapter that forwards to `AudioEncoderSink`, replacing `FfmpegExporter` on Android;
  - a way for `FsAudioCache` to return a file path;
  - a step-wise and resumable export runner;
  - pdfium bound by soname.

### 4.4 First milestones

| # | Milestone | Done when |
|---|---|---|
| **M1** | **Spike: Kokoro + pdfium on the user's real phone (go/no-go), ~1–2 weeks.** A bare Kotlin app (no design) + `lectrice-ffi` with `new`, `import_pdf`, `speak_sentence`. cargo-ndk for arm64-v8a; package `liblectrice_ffi.so`, `libsherpa-onnx-c-api.so`, `libonnxruntime.so`, `libpdfium.so`; push the model with `adb` into `filesDir`. | The APK builds and runs. A PDF imports. A sentence plays. **Measured and written down:** RTF for `af_heart` at 2/4/6 threads with fp32 and int8 (and xnnpack if it works); model load time; peak memory (`dumpsys meminfo`); temperature and RTF after 15 minutes of non-stop synthesis; import time of the largest book; APK size. **Gate:** RTF ≤ 0.6 → live read-ahead as on desktop. 0.6–1.0 → live at 1× plus "Prepare" mode. > 1.0 → "Prepare" mode first and evaluate a faster engine adapter. |
| **M2** | **Library + reader screen.** Design system from `tokens.css`; SAF import; recent list with progress and "continue"; reader (`LazyColumn` of blocks, page-marker chips, chapter label beside the page number, sentence highlight, tap to jump, follow-scroll + "locate"); chapters bottom sheet from `get_toc`; pill player UI wired to a fake `PlaybackController`. | Smooth scrolling on a 400-page book (JankStats; baseline profile added); light and dark; no main-thread core calls (StrictMode). |
| **M3** | **Background playback.** `MediaSessionService` + ExoPlayer + `SentenceDataSource`; read-ahead scaled by speed; speed and voice; position save; lock screen, notification, Bluetooth buttons, audio focus, headphones unplugged; survives screen off and process restart. | Listen to a whole chapter with the screen off, controlled from earbuds. |
| M4 | Export + model download + polish | Resumable M4B export through WorkManager within the 6-hour limit; first-run model download with SHA-256 check; open-source licenses screen; transitions polish. |

---

## 5. Risks and unknowns

1. **Kokoro speed on phones.** No published sherpa-onnx RTF for a current mid-range phone exists. A 2019 flagship was 1.13
   (fp32) and 2.42 (int8). Mitigation: the M1 gate, "Prepare" mode, a second engine adapter. Kokoro RTF on the user's phone is
   **UNVERIFIED**.
2. **Memory.** About 830 MB was reported on iOS for Kokoro. Android RSS is **UNVERIFIED**; 4 GB phones may kill the app.
   Load the engine lazily, keep one instance, and free it when idle.
3. **Heat and battery** during long synthesis: **UNVERIFIED**. Prefer "while charging" for big jobs.
4. **Rust Android build of sherpa-onnx-sys**, packaging and loading the `.so` files: **UNVERIFIED** end to end. The Kotlin AAR
   is the fallback (§3.6).
5. **pdfium loading with uncompressed native libs** (bind by soname vs path): **UNVERIFIED**.
6. **Foreground service limits.** Android 15+ allows 6 h per 24 h for `dataSync`/`mediaProcessing`, so export must be
   resumable. OEM battery killers (Xiaomi, Samsung) may stop background work earlier (**UNVERIFIED** per device).
7. **M4B chapters on Android** (MediaMuxer has no chapter support) need a post-processing step: **UNVERIFIED**. MP3 on mobile
   needs LGPL LAME, so it is deferred.
8. **GPL-3 espeak-ng inside sherpa's Android libs:** the APK must follow GPL-3. It may go away if sherpa-onnx 2.x really drops
   espeak-ng for Kokoro (**UNVERIFIED**).
9. **Model delivery:** about 370 MB on first run (or 126 MB int8, if it proves fast enough). The Play size limits differ between
   page snapshots (**UNVERIFIED**; check the live page if publishing on Play).
10. **Version churn:**
    - sherpa-onnx 2.0.0 shipped today, while the crate is still 1.13.8;
    - uniffi 0.32.2 (2026-09-23);
    - Media3 1.11.1 stable (1.12.0-alpha01 on 2026-10-21);
    - Kotlin 2.4.21;
    - Compose BOM, AGP and Navigation 3 versions were **UNVERIFIED** (Google Maven was not reachable). Pin them at spike time.
11. **Big document over FFI** (`get_document` for 400+ pages): the mapping cost is **UNVERIFIED**; page it if it is slow.
12. **iOS later:** the Rust core ports through uniffi Swift. The UI is a choice between SwiftUI and moving to Compose
    Multiplatform. Rust tooling for KMP (Gobley, last release 2025-10-08) may lag uniffi.

---

## 6. Glossary (Android terms)

| Term | Meaning |
|---|---|
| **Activity** | A screen-level entry point the OS starts. Modern apps use one and draw all screens inside it. |
| **Fragment** | Android's old "sub-screen inside an Activity". Telegram's `BaseFragment` is *not* one; it is its own class. |
| **View / ViewGroup** | The classic UI building block: an object that measures, lays out and draws itself (`onMeasure`/`onLayout`/`onDraw`). |
| **RecyclerView** | The classic list widget that reuses row Views while scrolling. |
| **Jetpack / AndroidX** | Google's official support libraries (Compose, Media3, WorkManager, Lifecycle…), versioned apart from the OS. |
| **Jetpack Compose** | Declarative UI in Kotlin (`@Composable` functions); the React-like way to build Android UI. |
| **Recomposition** | Compose re-running the composables whose state changed; the equivalent of a React re-render. |
| **Material 3** | Google's design system and component library for Compose. |
| **ViewModel / StateFlow** | A class that holds screen state and survives screen rotation, and the observable stream of that state. |
| **Coroutine / Dispatcher** | Kotlin's async tasks, and the thread pool they run on (`Main`, `IO`, `Default`). |
| **Service / foreground service** | Work that runs without a visible screen. "Foreground" means it shows a notification and the OS keeps it alive (types: `mediaPlayback`, `dataSync`…). |
| **MediaSession** | The OS-level "now playing" object that the lock screen, notification, Bluetooth and Android Auto talk to. |
| **Media3 / ExoPlayer** | Jetpack's media library: the player (ExoPlayer), sessions, and `MediaSessionService`. |
| **DataSource** | ExoPlayer's pluggable "read bytes for this URI" interface; ours will ask Rust for a sentence. |
| **WorkManager** | Jetpack's scheduler for deferrable, guaranteed background work (survives restarts; constraints like "charging"). |
| **SAF (Storage Access Framework)** | The system file picker; returns `content://` URIs instead of file paths. |
| **ABI** | CPU architecture for native code: `arm64-v8a` (almost all phones), `armeabi-v7a`, `x86_64` (emulators). |
| **NDK / JNI / jniLibs** | The native toolkit, Java's native-call interface, and the folder whose `.so` files get packaged per ABI. |
| **JNA** | A library that calls C functions from Java without hand-written JNI; uniffi's Kotlin bindings use it. |
| **uniffi** | Mozilla's generator of Kotlin/Swift bindings from Rust. |
| **cargo-ndk** | A cargo wrapper that builds Rust for Android ABIs with the NDK. |
| **APK / AAB** | The installable app file, and the bundle uploaded to Google Play (Play builds per-device APKs from it). |
| **R8 / baseline profile** | The shrinker and optimizer for Kotlin/Java code, and a list of hot code compiled ahead of time for faster startup and scrolling. |
| **minSdk / targetSdk** | The oldest Android version supported, and the version whose behaviour rules the app opts into. |
| **Gradle / AGP** | The build tool, and the Android Gradle Plugin. |
| **Jank / ANR** | Dropped frames, and "Application Not Responding" (the main thread blocked for too long). |
| **16 KB page size** | Newer devices use 16 KB memory pages; native `.so` files must be aligned for them (Play requirement). |
| **Play Asset Delivery** | Google Play's way to ship large files (like models) next to an app bundle. |

---

## Sources

- **Telegram:** https://github.com/DrKLO/Telegram (master, pushed 2026-10-20). Files read: `build.gradle`, `settings.gradle`,
  `gradle.properties`, `.gitmodules`, `README.md`, `LICENSE`, `TMessagesProj/build.gradle`, `TMessagesProj/src/main/AndroidManifest.xml`,
  `TMessagesProj/src/main/java/org/telegram/ui/ActionBar/{BaseFragment,ActionBarLayout,INavigationLayout,Theme,ThemeDescription}.java`,
  `ui/Components/{RecyclerListView,LayoutHelper,CubicBezierInterpolator,AnimatedFloat}.java`, `ui/{ChatActivity,LaunchActivity}.java`,
  `ui/Cells/ChatMessageCell.java`, `messenger/{MusicPlayerService,NotificationCenter,DispatchQueue}.java`,
  `src/main/java/androidx/recyclerview/widget/`, `jni/CMakeLists.txt`, `jni/tgnet/`.
- **sherpa-onnx:** https://github.com/k2-fsa/sherpa-onnx.
  - Releases: https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.13.8 (assets `sherpa-onnx-v1.13.8-android.tar.bz2`,
    `sherpa-onnx-1.13.8.aar`, `…-android-static-link-onnxruntime.tar.bz2`), https://github.com/k2-fsa/sherpa-onnx/releases/tag/v2.0.0,
    https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models.
  - Files read: `sherpa-onnx/kotlin-api/Tts.kt`, `android/SherpaOnnxTtsEngine/.../TtsEngine.kt`, `sherpa-onnx/csrc/provider.cc`,
    `sherpa-onnx/csrc/session.cc`, and the `sherpa-onnx-sys-1.13.8/build.rs` from the local cargo registry.
  - Crate: https://crates.io/crates/sherpa-onnx (1.13.8, 2026-09-11).
- **pdfium:**
  - binaries: https://github.com/bblanchon/pdfium-binaries/releases (`chromium/7881` `pdfium-android-arm64.tgz` inspected;
    latest `chromium/8111`, 2026-10-19);
  - pdfium-render 0.9.4 README: https://github.com/ajrcarey/pdfium-render.
- **espeak-ng license:** https://github.com/espeak-ng/espeak-ng (GPL-3.0).
- **Benchmarks:**
  - https://voiceping.net/en/blog/research-offline-tts-eval/
  - https://github.com/muxable/rtchat/issues/1514, https://github.com/muxable/rtchat/issues/1500
  - https://github.com/k2-fsa/sherpa-onnx/discussions/3968
  - https://speechcentral.net/2026/10/17/kokoro-for-android-from-proof-of-concept-to-practical-offline-tts/
  - https://soniqo.audio/benchmarks/android
- **uniffi:**
  - https://mozilla.github.io/uniffi-rs/latest/kotlin/gradle.html (JNA ≥ 5.12.0 `@aar`, coroutines ≥ 1.6, thread-attach advice);
  - crate 0.32.2 (2026-09-23);
  - Firefox's Rust components: https://github.com/mozilla/application-services.
- **Android docs:**
  - Media3 background playback: https://developer.android.com/media/media3/session/background-playback
  - Media3 releases: https://github.com/androidx/media/releases (1.11.1 stable)
  - Foreground service timeout: https://developer.android.com/develop/background-work/services/fg-service-timeout
  - Play size limits: https://support.google.com/googleplay/android-developer/answer/9859372
- **Cross-platform:**
  - flutter_rust_bridge: https://crates.io/crates/flutter_rust_bridge (2.14.0-beta.2)
  - uniffi for React Native: https://hacks.mozilla.org/2024/12/introducing-uniffi-for-react-native-rust-powered-turbo-modules/
    and https://github.com/jhugman/uniffi-bindgen-react-native (0.31.0-6)
  - Gobley: https://github.com/gobley/gobley (v0.3.7)
  - Compose Multiplatform for iOS stable (1.8.0): https://blog.jetbrains.com/kotlin/2025/05/compose-multiplatform-1-8-0-released-compose-multiplatform-for-ios-is-stable-and-production-ready/
  - Tauri: https://crates.io/crates/tauri (2.12.5)
  - cargo-ndk: https://crates.io/crates/cargo-ndk (4.1.2)
  - JNA: Maven Central (5.19.1)
  - Kotlin: Maven Central (2.4.21)
- **Local:** `CLAUDE.md`, `docs/2026-10-21-design-spec.md`, `docs/progress/B0.md`, `research/2026-10-06-rust-stack.md`,
  `docs/2026-10-23-chapters-brief.md`, `frontend/src/styles/tokens.css`, `backend/crates/lectrice-domain/src/ports.rs`,
  `models/kokoro-multi-lang-v1_0/` (file sizes).

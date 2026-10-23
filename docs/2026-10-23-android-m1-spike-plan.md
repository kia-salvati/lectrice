# Android M1: the go/no-go spike (plan, 2026-10-23)

Goal: find out, on the user's real phones, whether **Kokoro + pdfium + our Rust core** run well enough on a phone to build the
app as specced (`docs/2026-10-23-android-spec.md`). One to two weeks. **No design and no product UI.** The spike app is thrown
away afterwards (or kept as the `spike/` bench module).

Anything not checked is marked **UNVERIFIED**.

---

## 1. What the spike answers

1. Does the native stack build and load on both phones? That's `liblectrice_ffi.so` + `libsherpa-onnx-c-api.so` +
   `libonnxruntime.so` + `libpdfium.so`, built with cargo-ndk and called through uniffi/JNA.
2. How fast is Kokoro on each phone (real-time factor, RTF), for **fp32 and int8**, at **2 / 4 / 8 threads**, cold and after
   10 minutes of non-stop work?
3. How much memory does it take at peak? Does the phone heat up and slow down over 10 minutes?
4. How long does importing (cleaning) the largest book take on a phone? How long does it take to hand a whole document to Kotlin?
5. How big is the APK?
6. A smoke test of the real path: import one PDF via the system picker → clean it with the Rust core → synthesize about
   2 minutes of speech → play it through ExoPlayer with our own `DataSource`.
7. **sherpa-onnx 1.13.8 vs 2.0.0** (2.0 is GPL-free): speed on both phones, and (laptop only) how many words 2.0 skips in
   the user's books after our simple fixes. Is that rate "acceptable" as spec §14.2 defines it?
8. **Accelerators:** do the `xnnpack` or `nnapi` providers make Kokoro faster or cheaper on battery on either phone
   (spec §15)? What does each cost in battery per minute of audio, and in heat?
9. Honest numbers for the two model choices the user will see on the first-run screen (spec §6.1): full quality (fp32) vs
   smaller (int8).

---

## 2. The phones

| | Galaxy S10+ (2019) | Galaxy S26 Ultra |
|---|---|---|
| Chip | **Exynos 9820** or **Snapdragon 855**, depending on the region | Snapdragon 8 Elite Gen 5 "for Galaxy" (UNVERIFIED) |
| Cores | Exynos: 2 big (M4) + 2 mid (A75) + 4 small (A55). Snapdragon: 1 + 3 A76 + 4 A55 | 2 + 6 Oryon, no small cores (UNVERIFIED) |
| RAM | 8 GB (12 GB on the 1 TB model) | 12 GB or more (UNVERIFIED) |
| Last Android | 12 (One UI 4.1): API 31/32, 4 KB pages, **no** 6-hour foreground limit | 16 (UNVERIFIED): has the limit, may use 16 KB pages |

**How the user can check which S10+ it is:**

- **On the phone:** Settings → About phone → **Model number**.
  - `SM-G975F`, `SM-G975F/DS` (Europe/international) or `SM-G975N` (Korea) = **Exynos 9820**.
  - `SM-G975U`, `SM-G975U1` (US), `SM-G975W` (Canada), `SM-G9750` (China/Hong Kong) or `SC-04L`/`SCV42` (Japan) =
    **Snapdragon 855**.
  - (This list is from memory: UNVERIFIED. The command below is the reliable check.)
- **With adb (reliable):** `adb shell getprop ro.board.platform` prints `exynos9820` or `msmnile` (the Snapdragon 855 code name).
- The spike app also records `Build.SOC_MANUFACTURER` / `Build.SOC_MODEL` (API 31+, so both phones have it) in every result file.

The only published Kokoro number on a phone is for the **Exynos** S10: RTF 1.13 (fp32) and 2.42 (int8) at 4 threads (report
§3.4). So the S10+ result is expected to be close to, or slower than, real time.

---

## 3. What to install on the laptop (Fedora 44)

**Not now: the user is on a metered network (2026-10-23).** Nothing is downloaded or installed until the user runs the
copy-paste block in §3a on an unmetered network. The table below is what that block installs.

**Recommendation: the command-line SDK only, no Android Studio for the spike.** Agents drive everything from the shell, and
the user works through RustDesk, where a heavy IDE is slow. Android Studio is optional later (M2+) if the user wants its
layout inspector and profiler. It installs without sudo too.

**No sudo is needed for any of it.** Everything goes into the home folder. The one possible exception is USB permissions
(§4.1); wireless adb avoids it.

Already on the laptop: Rust 1.99.1 (rustup), 76 GB free on `/home`, 13 GB RAM, 16 threads, OpenJDK 25 **headless** (it has no
`javac`, so Gradle can't use it as a JDK).

| # | Item | Version (pin at install time) | Where | Download → on disk |
|---|---|---|---|---|
| 1 | **JDK 21** (Eclipse Temurin tarball) | `21.0.13+9` (Adoptium API, checked 2026-10-23) | `~/.local/opt/jdk-21` | 208 MB → ~330 MB |
| 2 | Android **command-line tools** (`sdkmanager`) | the current `commandlinetools-linux-*_latest.zip` (exact build UNVERIFIED: developer.android.com refused our requests) | `~/Android/Sdk/cmdline-tools/latest` | ~150 MB → ~300 MB |
| 3 | `platform-tools` (adb) | latest | `~/Android/Sdk/platform-tools` | ~15 MB → ~40 MB |
| 4 | `platforms;android-36` (or the newest stable API `sdkmanager --list` shows) | 36+ (UNVERIFIED if 37 is out) | SDK | ~70 MB → ~200 MB |
| 5 | `build-tools;36.x.y` (the newest stable) | UNVERIFIED | SDK | ~60 MB → ~150 MB |
| 6 | **NDK** | the newest **stable** from `sdkmanager --list`: r29 = `29.0.14206865` was the latest stable in cached pages, r30 was beta (UNVERIFIED today). Must be ≥ r28 (16 KB alignment by default). | `~/Android/Sdk/ndk/<ver>` | ~700 MB → ~2.3 GB (UNVERIFIED) |
| 7 | Rust target `aarch64-linux-android` | with Rust 1.99.1 | rustup | ~60 MB → ~200 MB (UNVERIFIED) |
| 8 | **cargo-ndk** | `4.1.2` (crates.io) | `~/.cargo/bin` | builds in ~1 min, ~10 MB |
| 9 | Gradle (through the wrapper) | `9.8.2` (services.gradle.org, current on 2026-10-23); AGP must support it (AGP version UNVERIFIED: pick the newest stable AGP 9.x and the Gradle it asks for) | `~/.gradle` | ~150 MB + caches **~2–3 GB** (AGP, Kotlin, Compose, Media3) |
| 10 | Kotlin | `2.4.21` (Maven Central) | Gradle | in the caches |
| 11 | Libraries | Media3 `1.11.1`, JNA `5.19.1` (`@aar`), uniffi `0.32.2`; Compose BOM / Activity / Lifecycle / WorkManager: the newest stable (UNVERIFIED; Google Maven wasn't reachable from here) | Gradle | in the caches |
| 12 | Native libs | sherpa-onnx Android archives (`v1.13.8`: 46.1 MB; `v2.0.0`: 45.2 MB; both needed), pdfium-binaries `chromium/7881` `pdfium-android-arm64.tgz` (6.3 MB unpacked) | `lectrice-android/native/` (gitignored) | ~55 MB → ~150 MB |
| 13 | int8 model, for comparison | `kokoro-int8-multi-lang-v1_0.tar.bz2` (sherpa `tts-models` release) | `lectrice/models/` (gitignored) | 132.3 MB → ~170 MB |
| 14 | Rust build output for Android | | `lectrice-android/target/` | ~2–4 GB |

**Total: about 8–10 GB** without Android Studio. Android Studio would add about 1.3 GB to download and 3–4 GB on disk. There
is plenty of room (76 GB free).

Environment (added to `~/.zshenv` by the install agent, after showing the user the lines):

```sh
export JAVA_HOME="$HOME/.local/opt/jdk-21"
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/<version>"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
```

`sdkmanager --licenses` needs the user to accept the SDK licenses. The agent shows them; the user types "y" (or tells the agent
to accept).

### 3a. Copy-paste "download later" block (run on an unmetered network)

**Total download: about 1.6 GB** for the tools and archives (sizes below). The first Gradle build adds about 2–3 GB more
(step S4; do that on an unmetered network too), and the first build of the `sherpa2` core branch downloads the
sherpa-onnx 2.0.0 crate plus its prebuilt host library (size UNVERIFIED, about 20–30 MB; steps S3/S6). Exact sizes come from the GitHub API (2026-10-23) where given; the others are
estimates. The block **resolves "latest" versions itself** where we couldn't verify them, and prints what it chose. It needs
no sudo.

**How to run it:** save it as `~/Downloads/lectrice-android-dl.sh` and run `bash ~/Downloads/lectrice-android-dl.sh`. Don't
paste it straight into zsh: `set -e` would close the terminal on the first error. It stops at the first failure, so it is
safe to fix the cause and run it again.

```sh
#!/usr/bin/env bash
set -euo pipefail
DL="$HOME/Downloads/lectrice-android-dl"; mkdir -p "$DL" "$HOME/.local/opt" "$HOME/Android/Sdk"; cd "$DL"

# --- 1. JDK 21, Eclipse Temurin (~208 MB). Resolves the newest 21.x and checks its SHA-256. ---
JDK_JSON=$(curl -fsSL "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jdk&os=linux&vendor=eclipse")
JDK_URL=$(printf '%s' "$JDK_JSON" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)[0]["binary"]["package"]["link"])')
JDK_SHA=$(printf '%s' "$JDK_JSON" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)[0]["binary"]["package"]["checksum"])')
curl -fL -o jdk21.tar.gz "$JDK_URL"
echo "$JDK_SHA  jdk21.tar.gz" | sha256sum -c -
rm -rf "$HOME/.local/opt/jdk-21"; mkdir -p "$HOME/.local/opt/jdk-21"
tar -xzf jdk21.tar.gz -C "$HOME/.local/opt/jdk-21" --strip-components=1
export JAVA_HOME="$HOME/.local/opt/jdk-21"; "$JAVA_HOME/bin/java" -version

# --- 2. Android command-line tools (~150 MB). ---
# This build number is UNVERIFIED as still hosted. If the download fails, copy the current file name from
# https://developer.android.com/studio#command-line-tools-only. Any recent build works: step 3 updates itself to "latest".
CMDLINE_ZIP="commandlinetools-linux-13114758_latest.zip"
curl -fL -O "https://dl.google.com/android/repository/$CMDLINE_ZIP"
export ANDROID_HOME="$HOME/Android/Sdk"
rm -rf "$ANDROID_HOME/cmdline-tools/bootstrap" /tmp/lectrice-cmdline && mkdir -p "$ANDROID_HOME/cmdline-tools" /tmp/lectrice-cmdline
unzip -q "$CMDLINE_ZIP" -d /tmp/lectrice-cmdline && mv /tmp/lectrice-cmdline/cmdline-tools "$ANDROID_HOME/cmdline-tools/bootstrap"

# --- 3. SDK packages: licenses, latest cmdline-tools + platform-tools (adb), then the newest STABLE platform,
#        build-tools and NDK (~900 MB in total; the NDK is most of it). ---
SDKM="$ANDROID_HOME/cmdline-tools/bootstrap/bin/sdkmanager"
"$SDKM" --sdk_root="$ANDROID_HOME" --licenses          # read and answer y
"$SDKM" --sdk_root="$ANDROID_HOME" "cmdline-tools;latest" "platform-tools"
SDKM="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
LIST=$("$SDKM" --list 2>/dev/null | awk '/Available Packages:/{a=1} a')
PLATFORM=$( { printf '%s\n' "$LIST" | grep -oE '^ *platforms;android-[0-9]+ ' || true; } | tr -d ' ' | sort -t- -k2 -n | tail -1)
BUILD_TOOLS=$( { printf '%s\n' "$LIST" | grep -oE '^ *build-tools;[0-9]+\.[0-9]+\.[0-9]+ ' || true; } | tr -d ' ' | sort -V | tail -1)
NDK=$( { printf '%s\n' "$LIST" | grep -oE '^ *ndk;[0-9]+\.[0-9]+\.[0-9]+ ' || true; } | tr -d ' ' | sort -V | tail -1)
[ -n "$PLATFORM" ] && [ -n "$BUILD_TOOLS" ] && [ -n "$NDK" ] || { echo "Could not resolve SDK versions; run: $SDKM --list"; exit 1; }
echo "Chosen: $PLATFORM | $BUILD_TOOLS | $NDK"
printf '%s\n' "$LIST" | grep -E "^ *(${NDK}|${BUILD_TOOLS}) " || true   # check that the description says nothing like rc/beta
"$SDKM" "$PLATFORM" "$BUILD_TOOLS" "$NDK"
"$SDKM" --list_installed

# --- 4. Rust for Android (~60 MB) and cargo-ndk 4.1.2 (a few MB of crates). ---
rustup target add aarch64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked

# --- 5. Native archives for the spike, checked against the SHA-256 that GitHub publishes (~95 MB). ---
NATIVE="$HOME/Desktop/learning/lectrice-android/native/downloads"; mkdir -p "$NATIVE"; cd "$NATIVE"
curl -fL -O https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-android.tar.bz2       # 46.1 MB
curl -fL -O https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.0/sherpa-onnx-v2.0.0-android.tar.bz2         # 45.2 MB
curl -fL -O https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7881/pdfium-android-arm64.tgz      # 3.3 MB
sha256sum -c - <<'SUMS'
2ff63469a71cb6009aa2e3ed5f4a670f8abdcbe4bb9ffd23776afc792a6b4f44  sherpa-onnx-v1.13.8-android.tar.bz2
578650d0221255566bd7c88bd3a0deeb974b8562a332a6a875b35bd00b7e0f5d  sherpa-onnx-v2.0.0-android.tar.bz2
16d23bb86c4188d59326dc509938f59df3c417ecfdd3d2ca2f160dc5bd49a839  pdfium-android-arm64.tgz
SUMS

# --- 6. The int8 Kokoro model for comparison (132.3 MB; unpacks to ~170 MB next to the fp32 one). ---
cd "$HOME/Desktop/learning/lectrice/models"
curl -fL -O https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2
echo "4c3052abaa60943a341f193888cf6abd68787dae6ab8ae5c925a706caa247e4e  kokoro-int8-multi-lang-v1_0.tar.bz2" | sha256sum -c -
tar -xjf kokoro-int8-multi-lang-v1_0.tar.bz2 && rm kokoro-int8-multi-lang-v1_0.tar.bz2

# --- 7. Laptop desk check for the accelerators (spec §15.5): ONNX Runtime + onnx in their own venv (~60 MB, UNVERIFIED). ---
python3 -m venv "$HOME/.local/opt/ort-venv"
"$HOME/.local/opt/ort-venv/bin/pip" install onnxruntime onnx

echo "Done. Disk use:"; du -sh "$HOME/.local/opt/jdk-21" "$ANDROID_HOME" "$NATIVE" 2>/dev/null
```

Afterwards, add the environment lines above to `~/.zshenv` (with `ANDROID_NDK_HOME` set to the NDK folder the block chose).
The `native/downloads/` folder must be gitignored in `lectrice-android` (task S3 adds it; today's `.gitignore` doesn't have
it).

---

## 4. Connecting the phones

### 4.1 Options

| Way | Needs | Good for | Watch out |
|---|---|---|---|
| **Wireless debugging** (Android 11+; both phones have it) | Phone and laptop on the **same Wi-Fi** | **Recommended for measuring:** the phone isn't charging, so temperature and battery numbers are honest; no USB permission issues | Pair once with a code; the port changes after a reboot |
| USB cable to the laptop | The phone next to the laptop | First install, fast pushes (the 350 MB model) | It **charges the phone**, which warms it and skews the heat test. On Fedora, `adb devices` may say `no permissions`; then a udev rule is needed (sudo `dnf install android-tools`, which should bring one, UNVERIFIED) or use wireless |
| **No adb** (the phone is with the user, away from the laptop) | Copying the APK to the phone (e.g. RustDesk file transfer to the user's own computer, then to the phone) | When the user is not at home | The spike app runs every test by itself and **shares the result file** (Android share sheet → e-mail/Drive to the user) |

RustDesk controls the laptop only; it doesn't forward USB. So for adb the phone must be **physically at the laptop (USB)** or
**on the laptop's Wi-Fi (wireless)**. If the user is remote and the phones are at home with the laptop, wireless adb works
as long as both are on the home Wi-Fi.

### 4.2 One-time setup, per phone (the user does this; about 5 minutes)

1. Settings → About phone → Software information → tap **Build number** 7 times → enter the PIN. "Developer options" appears.
2. Settings → Developer options → turn on **USB debugging** and **Wireless debugging**.
3. Wireless: tap "Wireless debugging" → **"Pair device with pairing code"**. The phone shows an IP:port and a 6-digit code.
   On the laptop (an agent can type it): `adb pair <ip>:<pair-port> <code>`, then
   `adb connect <ip>:<port shown on the main Wireless debugging screen>`.
4. `adb devices -l` lists the phone. The agent records the serial, `ro.product.model` and `ro.board.platform`.
5. Optional, for the heat test: Developer options → **"Stay awake"** is only for charging, so instead the spike app keeps the
   screen on during a run (`FLAG_KEEP_SCREEN_ON`).
6. Copy the test PDFs onto the phone yourself (they are your books and never go into git):
   `adb push ~/Desktop/<book>.pdf /sdcard/Download/`, or any file transfer.

---

## 5. The spike app

A separate, minimal project inside the `lectrice-android` repo (already created: `../lectrice-android`, public on GitHub,
with its own `CLAUDE.md`), following spec §3.1: `rust/lectrice-ffi` (spike subset, feature `bench`) and one Gradle module
`spike/`. One Compose screen with plain buttons and a text log. **No design.**

**Two builds of the same app**, one per sherpa-onnx version (Gradle product flavors `sherpa1` and `sherpa2`, with different
application ids, so both can be installed side by side):

- `sherpa1`: the core as it is (sherpa-onnx 1.13.8, with `espeak-ng-data` in the model folder);
- `sherpa2`: the same core code with the workspace pin bumped to `sherpa-onnx = "=2.0.0"` (on a second core branch,
  `feature/android-spike-sherpa2`, used through its own path override). It uses the v2.0.0 Android libs, and the model folder
  without `espeak-ng-data`.

Cargo can't hold both versions in one build (both crates link the same native library), hence two builds.

### 5.1 Rust side (`lectrice-ffi`, spike subset)

From the spec's API (§5.2): `LectriceCore::new`, `engine_status`, `reload_engine`, `import_pdf`, `get_document`,
`playback_order`, `speak_sentence`, `list_voices`, plus `LogSink`. The other callbacks get no-op implementations. Plus, behind
the `bench` feature:

```rust
#[derive(uniffi::Record)] pub struct BenchResult { pub engine_load_ms: u64, pub sentences: u32, pub audio_ms: u64, pub synth_ms: u64,
    pub rtf: f64, pub first_sentence_ms: u64, pub per_sentence: Vec<BenchSample> }
#[derive(uniffi::Record)] pub struct BenchSample { pub index: u32, pub chars: u32, pub audio_ms: u32, pub synth_ms: u32, pub at_ms: u64 }
#[uniffi::export] impl LectriceCore {
    pub fn bench_synthesize(&self, texts: Vec<String>, voice_id: String) -> Result<BenchResult, CoreError>; // no cache, straight to the engine
}
```

- **Bench text:** about 20 invented sentences, about 1 minute of audio (written for the spike; any wording, **not from any book**). Committed as
  `spike/src/main/assets/bench-text.txt`, so every run and every phone speaks the same text.
- **Core changes for the spike:** only what's needed to build without ffmpeg (spec C1) and to pass the provider (C6). Done on
  a `feature/android-spike` branch in the main repo, used through the path override (spec §3.3). There is no tag yet.

### 5.2 Kotlin side (`spike/`)

- `minSdk 29`, `arm64-v8a` only, release build with R8, signed with the debug key (it's a test app).
- **Buttons:**
  1. **Import PDF:** SAF `OpenDocument` → copy to `cacheDir` → `import_pdf`. It records the copy time, the import time, the
     page and sentence counts, then the `get_document` time and record count.
  2. **Engine:** pick fp32/int8, 2/4/8 threads and the provider (`cpu`, `xnnpack`, `nnapi`) → `reload_engine`. Records the
     load time, and whether the provider really engaged: the sherpa log lines ("Use nnapi", "Fallback to cpu", "Available
     providers: …") go through `LogSink` into the result file.
  3. **Bench:** `bench_synthesize` with the bench text, for **every combination** in turn: {fp32, int8} × {2, 4, 8} threads on
     `cpu`, each run 3 times, the first discarded as warm-up. On the S26 Ultra, also 6 threads. (The `sherpa2` build runs only
     the best thread count from `sherpa1`, for fp32 and int8.)
  3b. **Accelerators:** at that phone's best thread count, for fp32 and int8: `xnnpack`, then `nnapi`, each run 3 times (first
     discarded). For each run it records RTF, whether the provider engaged, and the **clip durations** compared with the
     `cpu` run (they must agree within ±3 %; samples can't be compared byte for byte because Kokoro's graph has random-noise
     ops). It saves one sentence's WAV per provider so the user can listen. Any provider that engages **and** is at least as
     fast as `cpu` also gets a 5-minute battery run like (4).
  4. **Soak 10 min:** the best combination from (3), with the bench text on a loop for 10 minutes. Every 10 s it records:
     RTF of the last sentences, `PowerManager.currentThermalStatus`, `getThermalHeadroom(10)` (API 30+), battery temperature
     (`ACTION_BATTERY_CHANGED` `EXTRA_TEMPERATURE`), battery %, whether it's charging, `VmRSS`/`VmHWM` from `/proc/self/status`,
     and `BatteryManager.BATTERY_PROPERTY_CHARGE_COUNTER` (µAh) and `CURRENT_NOW`. From these it works out **mAh per minute
     of audio produced**. (Samsung may report `CURRENT_NOW` in mA instead of µA: UNVERIFIED; the charge counter is the main
     number.) It refuses to start while charging, because the battery numbers would be meaningless.
  5. **Play 2 min:** from the imported PDF, starting at the first chapter: an ExoPlayer playlist of the next sentences
     (`playback_order`) through a minimal `SentenceDataSource` that calls `speak_sentence` (spec §7.3). It plays until about
     2 minutes of audio have played, then records the gaps: time from the end of one sentence to the start of the next,
     buffering events, and time to the first sound.
  6. **Run all:** 2 → 3 → 3b → 4 in a row, keeping the screen on. About 60–90 minutes per phone and build; it can be split
     across sessions (each part saves its own results).
  7. **Share results:** the JSON file through the share sheet. The agent can also fetch it with `adb shell run-as <pkg> cat files/results/<file>.json`.
- **Result file** (`files/results/<device>-<timestamp>.json`): device (model, SoC, RAM, Android/API, page size, ABI), app and
  core versions, model bundle, threads, every number above. **Only ids, counts and durations: no book text, no PDF file
  names, no titles.** That way the files can be committed.
- **Model on the phone:** `spike/push-model.sh <serial> fp32|int8` pushes the bundle to `/data/local/tmp/`, then
  `adb shell run-as <pkg> cp -r` it into `files/models/<bundle>/`. (`run-as` works because the spike is debuggable; for a
  release build, the agent installs a `debuggable` variant just for pushing.) No download code in the spike.

### 5.3 What to measure, per phone

| # | Measurement | How |
|---|---|---|
| M-a | Native libs load; sherpa engine created; pdfium binds by **bare name** (and, if not, by full path with legacy packaging) | log + result file |
| M-b | **RTF** (synthesis time ÷ audio time) for fp32 and int8 at 2/4/8 threads (+6 on the S26), median of 2 runs | Bench |
| M-c | Engine load time, cold (after a reboot) and warm | Engine |
| M-d | First-sentence latency (press → first sound) | Play 2 min |
| M-e | **Peak memory**: `VmHWM` during the bench, plus `adb shell dumpsys meminfo <pkg>` (PSS) when an agent is connected | Bench, soak |
| M-f | **Heat / throttling over 10 minutes**: thermal status and headroom, battery temperature, and RTF at minute 1 vs minute 10, **unplugged** | Soak |
| M-g | Battery used in 10 minutes (%) | Soak |
| M-h | **PDF import time** for the largest of the user's books, plus a medium one; `get_document` time | Import |
| M-i | Gaps between sentences while playing; buffering events | Play 2 min |
| M-j | **APK size** (release, arm64) and the size of each `.so` | `ls -l`, `unzip -lv` on the laptop |
| M-k | sherpa 2.0 vs 1.13.8: words skipped per book, before and after our simple fixes (laptop only, task S6) | counts only |
| M-l | sherpa 2.0 vs 1.13.8: RTF at the best thread count, fp32 and int8 | Bench (`sherpa2` build) |
| M-m | **Accelerators:** per provider, whether it engaged, RTF, durations vs `cpu`, a listen | Accelerators |
| M-n | **Battery:** mAh per minute of audio for `cpu` (soak), and for any provider that engaged and was not slower | Soak, accelerators |
| M-o | Laptop desk check: how much of `model.onnx` NNAPI could take (ORT's `check_onnx_model_mobile_usability`) | laptop |

---

## 6. Pass / fail thresholds

The RTF that counts is the **sustained** one (the median over minutes 8–10 of the soak), for the best combination on that
phone.

| Check | Pass (green) | Acceptable (amber) | Fail (red) |
|---|---|---|---|
| Native build and load (M-a) | Everything loads on both phones | Works only with legacy packaging / full paths | Can't load → fallback F1 |
| **Sustained RTF** (M-b, M-f) | **≤ 0.6**: live read-ahead like the desktop, even at 1.5×; **no Prepare mode** | **0.6–0.9**: live at 1×; Prepare mode is built for > 1.2× | **> 0.9**: can't keep up live → Prepare is built and comes first |
| First sentence (M-d) | ≤ 1.5 s | ≤ 3 s | > 3 s |
| Engine load (M-c) | ≤ 3 s | ≤ 8 s | > 8 s |
| Peak memory (M-e) | VmHWM ≤ 1.0 GB | ≤ 1.5 GB | > 1.5 GB or killed by the system |
| Heat (M-f) | Thermal status ≤ `MODERATE`, RTF drops ≤ 15 % | ≤ `SEVERE`, drop ≤ 35 % | `CRITICAL`, or drop > 35 % |
| Import of the largest book (M-h) | ≤ 15 s | ≤ 45 s (needs a progress UI) | > 45 s |
| `get_document` (M-h) | ≤ 0.5 s | ≤ 1.5 s (add paging, spec §5.3) | > 1.5 s |
| Gaps while playing (M-i) | No audible gap after the first sentence (cache warm) | Short gaps only at the start | Regular buffering |
| APK, arm64 release (M-j) | ≤ 50 MB | ≤ 70 MB | > 70 MB (look at the static onnxruntime variant) |
| sherpa 2.0 skipped words (M-k, after fixes) | average ≤ 0.1 % **and** worst book ≤ 0.25 % **and** the ear test passes (spec §14.2) | | anything above → stay on 1.13.8 for now |
| sherpa 2.0 speed (M-l) | within ±10 % of 1.13.8 | 10–25 % slower | > 25 % slower → find out why before switching |
| An accelerator (M-m, M-n) | engages, durations agree, sounds right, **and** ≥ 15 % faster or ≥ 15 % fewer mAh per audio-minute → make it that phone's default | engages but no clear gain → keep `cpu` | doesn't engage, or is slower, or sounds wrong → drop it |

**The decision:**

- **GO** if the **S26 Ultra** is green or amber on RTF and passes the hard checks (load, memory, import). The S10+ result then
  sets the **minimum phone class**: green/amber there = live mode for everyone; red there = older phones get Prepare-first, and
  the app says so on its first run.
- **GO with Prepare-first** if the S26 Ultra itself is red on RTF but everything else passes. Listening then plays mostly from
  audio generated ahead (while charging). This changes M3/M4 priorities, not the architecture.
- **NO-GO for Kokoro on phones** only if even "Prepare while charging" can't make an hour of audio in under about 2 hours on the
  S26 Ultra (RTF > 2), or memory kills the app. Then try F3/F4 before giving up.

**Model choice notes:** the spike's fp32 and int8 numbers per phone become the one-line notes on the first-run screen (spec
§6.1); for example "real-time on this phone" vs "a little slower than real time". The user picks; we never pick for them.

**sherpa-onnx version:** if 2.0 passes both rows above, the core moves to 2.0 (spec C6) for both apps. Otherwise it stays on
1.13.8 and the APK carries GPL-3 (spec §14.3).

**Accelerators, honest expectation** (spec §15): `xnnpack` about the same as `cpu`; `nnapi` no gain, or a silent fallback to
the CPU; QNN (the Snapdragon NPU) and the GPU can't be tried in M1, since sherpa doesn't expose them for TTS. They become a
separate follow-up (spec §15.6) only if the CPU result is amber/red or the battery drain is bad.

**On-device fallbacks, in order** (no server, by the user's decision):

- **F1 (build fails):** use sherpa's official Kotlin AAR inside a Kotlin `SpeechEngine` adapter that Rust calls as a uniffi
  foreign trait (report §3.6). The architecture stays the same.
- **F2 Prepare while charging:** generate chapters or the whole book ahead (spec §8.5). Built **only if** the RTF row says so
  (user decision 2026-10-23).
- **F3 Tune the engine:** int8 where it's faster, the best thread count per phone (or the best per watt), and an accelerator
  only if it passed the row above.
  Shorter sentences don't change RTF, but they lower the first-sentence wait.
- **F4 A lighter engine:** another `SpeechEngine` adapter, e.g. the 7M Kokoro distills from the sherpa 2.0 notes, or
  KittenTTS nano. Each needs a license check, an OOV check and the user's ear test (UNVERIFIED that any sounds good enough).

---

## 7. Tasks for the agents

All agents are **Opus** (no visual design in M1). Each step writes a short report to `lectrice-android/docs/progress/<step>.md`.
Rules for all of them: no commits, no published Artifacts, **no book text in any file**, show the user any change to
`~/.zshenv` or any `sudo` command before running it.

| Step | Who | What | Done when |
|---|---|---|---|
| **S0** | user | Phone setup (§4.2): developer options, wireless pairing; read out the S10+ **model number**; copy 2 PDFs onto each phone; say whether you'll be at home (adb) or away (share-file mode) during the runs | Both phones show in `adb devices` (or share mode is agreed) |
| **S1** | **user** (agent helps) | **Install, on an unmetered network only**: run the §3a block. Then the agent adds the environment lines to `~/.zshenv` (shown to the user first), writes the versions the block chose into `lectrice-android/docs/versions.md` and `gradle/libs.versions.toml`, and measures the real disk use (`du -sh`) | `sdkmanager --list_installed`, `cargo ndk --version` and `adb version` work; versions and disk use are written down |
| **S2** | agent | **Core prep** (main repo, branch `feature/android-spike`): spec C1 (infra features; desktop defaults unchanged) and C6 (the provider setting; on the second branch `feature/android-spike-sherpa2`, the pin bumped to sherpa-onnx 2.0.0 and the `espeak-ng-data` check dropped). Run the desktop's full `cargo test`, `clippy` and `fmt` with default features, and `cargo build -p lectrice-infra --no-default-features --features pdfium,kokoro` | Desktop unchanged and green; infra builds without ffmpeg |
| **S3** | agent | **ffi in the existing repo** (`../lectrice-android`, no commit): the `.cargo/config.toml` path override, `rust/lectrice-ffi` (spike subset + `bench`), `native/fetch-libs.sh` + `SHA256SUMS` (the three archives from §3a; it unpacks from `native/downloads/` and downloads only what's missing), and `.gitignore` lines for `native/downloads/` and `**/jniLibs/`. Also the second core branch for the `sherpa2` build (§5). Host build + a host smoke test (import an invented PDF, speak a sentence, on the laptop). Then `cargo ndk -t arm64-v8a --platform 29 build -p lectrice-ffi --release`, then generate the Kotlin bindings | `liblectrice_ffi.so` for arm64 plus the Kotlin bindings exist; `readelf -d` shows only the expected `NEEDED` libs; alignment is `0x4000` |
| **S4** | agent | **Spike app** (§5.2): Gradle project with the `sherpa1`/`sherpa2` flavors, JNA `@aar`, jniLibs, the buttons, the result JSON, `push-model.sh`, the invented bench text. Build both release APKs. Record the APK sizes (M-j). **The first Gradle build downloads 2–3 GB: unmetered network only.** | `./gradlew :spike:assembleSherpa1Release :spike:assembleSherpa2Release` works; both APKs install on one phone and the engine loads |
| **S5** | agent + user | **Run on the S26 Ultra**: push both models, Import (largest + medium book), Run all on `sherpa1`, then the short run on `sherpa2`, Play 2 min (user listens: gaps? voice OK? and the saved accelerator sentences), Share/pull results. Unplugged for the soak and battery runs (wireless adb) | The result JSON is in `lectrice-android/docs/results/` |
| **S6** | agent (laptop only, in parallel with S4/S5) | **OOV check**: a dev command (main repo, `lectrice-api` dev mode or a test binary) that cleans the 5 books from `~/Desktop/*.pdf` and counts, per book, the spoken words the sherpa 2.0 Kokoro lexicon front end would skip (same normalisation as `offline-tts-kokoro-impl.h`). Output **counts only**: words total, skipped, skipped share, and how many skipped words are all-caps / contain digits / camelCase; then the same counts **after** the simple fixes from spec §14.2 (acronyms spelled, camelCase and digits split). Also synthesize 3 invented sentences with tricky words on 1.13.8 and 2.0 for the user's ear test. Also the accelerator desk check (M-o) with the §3a venv | A table of counts in `research/2026-10-2x-sherpa2-oov.md` (no book words), with the verdict against spec §14.2 |
| **S7** | agent + user | **Run on the S10+**: same as S5 | Its result JSON is in |
| **S8** | agent | **Report**: `research/2026-10-2x-android-m1-results.md`: tables per phone, the gate in §6 applied, threads per phone, the draft fp32/int8 notes for the first-run screen, the sherpa 2.0 verdict, the accelerator results (with battery per audio-minute), whether Prepare mode is needed, what changed vs the spec, and the decision for the user | The user reads it and decides GO / Prepare-first / fallback |

**Order:** S0, and S2 on the core (no downloads needed), can start now. S1 waits for an unmetered network. Then S3, S4. S6
can run any time after S1. Then S5 and S7, then S8.

**If something blocks:**

- `sherpa-onnx-sys` won't build for Android → set `SHERPA_ONNX_LIB_DIR` to the vendored libs. If it still fails → F1 (the AAR
  adapter), and write down why.
- pdfium doesn't bind by bare name → full path plus `useLegacyPackaging = true`, and write down which one worked.
- Gradle/AGP/JDK version clash → use the AGP's documented JDK (21) and the Gradle version AGP asks for; never JDK 25 for the
  build.

---

## 8. What the user gets at the end

- `research/2026-10-2x-android-m1-results.md` (the decision page) and `research/2026-10-2x-sherpa2-oov.md`.
- A spike APK on both phones that plays 2 minutes of one of their books.
- Exact versions and the real disk use in `lectrice-android/docs/versions.md`.
- The answers the spec waits for: the notes for the two model choices, sherpa 2.0 yes/no, Prepare mode needed or not, and
  accelerators worth it or not.

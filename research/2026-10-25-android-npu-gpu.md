# Kokoro on the phone's NPU or GPU, and CPU tuning per chip

2026-10-25. Research only: nothing built, no phone used, git untouched. Updates spec §15 (`docs/2026-10-23-android-spec.md`).
"UNVERIFIED" means I couldn't confirm it from a primary source today. Several Google pages (developers.google.com,
ai.google.dev) answered 403 to the fetch tool, so some LiteRT details come from GitHub and search snippets.

## 0. The short version

| Question | Short answer |
|---|---|
| Qualcomm NPU (Hexagon) | **Possible, but expensive.** Someone already did it: an Apache-2.0 app runs Kokoro's vocoder on the S24 Ultra's NPU at RTF ≈ 0.29 (vs 0.74 on its CPU). It takes model surgery, 11 fixed-length pieces compiled per NPU generation, Qualcomm's closed runtime libraries in the APK, and our own ORT session. sherpa-onnx can't do it for TTS. Weeks of work, Snapdragon-only. |
| GPU | **The surprise: there is now a ready path.** Google's `litert-community/Kokoro-82M` (Apache-2.0) runs Kokoro through LiteRT with the vocoder on the GPU. Measured on a **Galaxy S26 with the same chip as our S26 Ultra: RTF 0.12 on GPU, 0.26 on its CPU** (we get 0.47–0.63 with sherpa fp32). That is about 4× faster than us. |
| Exynos | S10+ (Exynos 9820): **no NPU path** (Samsung closed its NPU SDK to third parties; NNAPI doesn't fit Kokoro). Only Exynos 2500/2600 have an NPU route (LiteRT, ExecuTorch). The S10+'s **GPU** might work through LiteRT: worth one test. |
| CPU tuning | Use only the **big cores**: read core capacities from `/sys`, pin the engine thread to the big cores before creating the engine (ORT's workers inherit it), pick the thread count with a short self-benchmark. Run generation in a **foreground service** (the S10+ dropped to RTF 4–8 in the background). Our int8 model is slow because of how it is quantized, not because int8 is slow on these chips. |

**Recommendation:** first the cheap CPU fixes (days), then **test LiteRT's Kokoro on both phones** (a few days, no app
change needed). Only if LiteRT is good do we build a second `SpeechEngine` adapter on it. QNN (NPU) comes last, and only
if battery numbers say we need it.

---

## 1. Qualcomm NPU (Hexagon HTP)

### 1.1 What "NPU" means here

An NPU is a chip block built for neural-network math. It does that math with far less energy than the CPU, but it is
picky: it wants **fixed tensor shapes** (the size of every input known in advance) and prefers **small number formats**
(int8/int16, or fp16 on new chips). On Snapdragon it is called the Hexagon HTP, and Qualcomm's software for it is
**QNN** ("AI Engine Direct", shipped in the **QAIRT** SDK).

### 1.2 Ways to reach it

| Route | Status for us |
|---|---|
| **ONNX Runtime QNN execution provider** | Works on Android (ORT docs). Microsoft publishes `com.microsoft.onnxruntime:onnxruntime-android-qnn` on Maven Central; it depends on Qualcomm's `com.qualcomm.qti:qnn-runtime` (the closed QNN libraries). Spec §15.3 found 1.29.0 (2026-08-12); a search snippet listed 1.24.1 with `qnn-runtime` 2.42.0, so check the version when downloading. Since ORT 1.18 the QNN libs come with the package; no separate SDK at runtime. |
| **sherpa-onnx QNN** | **ASR only** (Paraformer, Whisper, SenseVoice, Zipformer, Moonshine, …). The 1.13.x and 2.0.0 release notes add more QNN ASR models, nothing for TTS. Not an option. |
| **LiteRT Qualcomm accelerator** (Google) | LiteRT's own QNN accelerator; supports SM8850 (our S26 Ultra), SM8750, SM8650 and older (LiteRT `vendors/qualcomm` README). Google wants it shipped through Play (AI Packs + Play Feature Delivery). We ship on GitHub Releases, so we'd have to bundle the libs ourselves (UNVERIFIED that this works outside Play). |
| **Qualcomm AI Hub** | A cloud service that compiles a model into a QNN "context binary" (a pre-compiled NPU program) for a given chip. **No Kokoro there.** It has **MeloTTS** (EN/JA/KO, MIT), split into encoder + flow + decoder, for 8 Elite and 8 Elite Gen 5. Needs a Qualcomm account. |

### 1.3 Someone already ran Kokoro on Hexagon

- **`cedgeremek/kokoro-offline-tts-android`** (Apache-2.0, 2026). Tuned on a Galaxy S24 Ultra (SM8650, HTP v75).
  - The text front end (misaki + espeak 1.52), the duration predictor, the "harmonic source" math and the final iSTFT
    stay on the **CPU**. Only the **1,153-node vocoder** runs on the NPU.
  - The vocoder is pre-compiled into **11 fixed lengths** (64 … 640 frames); each window of audio is padded to the
    next size, and windows are joined with 50 ms cross-fades.
  - Graph fixes were needed: 50 × `Pow(x, 2)` → `Mul(x, x)` (the NPU gave wrong numbers otherwise), and part of the
    source math stays on the CPU.
  - QNN runtime from Maven (`com.qualcomm.qti:qnn-runtime:2.48.0`), contexts built with QAIRT 2.48.
  - Samsung phones needed a device-probe fix (`adsprpc-smd` instead of `fastrpc-cdsp`).
  - **Speed:** RTF 0.29 over a whole article (p95 0.32), against 0.74 for its own int8 CPU path (2.4× faster). First
    audio 0.95 s. **No battery data** ("pending a controlled benchmark").
  - Its context binaries are for HTP **v75** and **won't load on the S26 Ultra's NPU** (QNN context binaries are tied to
    one Hexagon generation). We'd have to recompile for the S26's HTP (v81, UNVERIFIED).
- **`MansfieldPlumbing/Kokoro-Hexagon`** (Apache-2.0): hand-written Hexagon DSP kernels, no QNN, no ORT. The generator
  runs ~20–30× real time on an S23 (SM8550), but the end-to-end pipeline isn't finished. A research project, not
  something we can use.

### 1.4 What Kokoro's graph needs for the NPU

From spec §15.3 and the S6 desk check, plus the ports above:

- **Dynamic lengths** must become **fixed buckets + padding** (exactly what cedgeremek did). The output length is only
  known after the duration predictor, so the model has to be **split**: front part (text encoder, LSTMs, duration
  predictor) on the CPU, vocoder in buckets on the NPU.
- **Unsupported ops** in QNN EP: `LSTM`, `Loop`, `If`, sequence ops, `ScatterND`, `CumSum`, `Range`, random ops. All
  of them sit in the front part, which stays on the CPU anyway. `InstanceNormalization` and `ConvTranspose` are
  supported.
- **Precision:** HTP runs quantized models (uint8/uint16 QDQ, needs calibration data and an x86 machine) or, on newer
  chips, fp32 graphs at **fp16** (`enable_htp_fp16_precision`, default on). Kokoro's vocoder is **fp16-sensitive**: Google
  had to re-export it "for FP16 safety" because the old one produced noise on GPUs at fp16; Kokoro-Hexagon reports its
  w8a16 compile "does not finalize". Expect numerical work.
- **Load time:** compiling on the phone is slow; ship pre-compiled **context binaries** (`ep.context_enable=1`), one
  set per HTP generation.

### 1.5 Licensing of the QNN libraries (important for a public APK)

- `com.qualcomm.qti:qnn-runtime` on Maven Central declares the **"Qualcomm AI Hub Model License"** (a PDF on Qualcomm's
  site), not an open-source license. Source isn't public.
- Third-party summaries say the Qualcomm license lets you ship the runtime **only as object code inside your app**,
  with Qualcomm's notices kept, and no reverse engineering. **UNVERIFIED**: I couldn't read the license text.
  react-native-sherpa-onnx chooses not to ship the QNN libs at all (each app developer adds them).
- Our repos are MIT OR Apache-2.0, and the APK already carries GPL-3 espeak-ng. Adding a proprietary blob is probably
  allowed, but **someone must read the actual license PDF before any release**. The CPU build must keep working
  without it.

### 1.6 Expected gain and risks

- Gain: about 2–3× over a good CPU path for the vocoder (cedgeremek), and NPUs usually use much less energy per
  operation. Nobody has published Kokoro battery numbers, so the energy gain is **UNVERIFIED**.
- Risks: Snapdragon-only; one compile per Hexagon generation; fp16/int numerical bugs; Samsung driver quirks; closed
  libraries; we'd need our own Kokoro front end (sherpa doesn't hand out phonemes) and our own ORT session; Qualcomm
  account + QAIRT download for compiling.
- **On the S26 Ultra, LiteRT's GPU number (RTF 0.12) is already better than cedgeremek's NPU number on the S24 Ultra
  (0.29).** So the NPU only makes sense if it clearly wins on battery.

---

## 2. GPU

### 2.1 ONNX Runtime

- No OpenCL/Vulkan EP for Android. NNAPI is deprecated and covers ~46 % of Kokoro in 64+ pieces (S6 desk check). The
  WebGPU EP exists but isn't packaged for Android apps or exposed by sherpa (spec §15.3). QNN's GPU backend: Kokoro
  coverage UNVERIFIED. **No ready GPU path through ORT.**

### 2.2 LiteRT (the new name of TensorFlow Lite): the ready path

`huggingface.co/litert-community/Kokoro-82M` (Apache-2.0; lexicons MIT/BSD):

- One `.litertlm` file per variant (acoustic model + vocoder + 54 voices + text front end). Recommended:
  `kokoro_int8_gpu_fp16.litertlm`, **295 MB**. Acoustic model on the CPU (int8 weights, the six LSTMs fused into one
  custom op), vocoder on the GPU at fp16.
- About 124 phonemes (~10 s of speech) per run; longer text is split at punctuation, output is streamed.
- Runs through **LiteRT-LM "Omni"** (`github.com/google-ai-edge/LiteRT-LM/omni`, Apache-2.0, experimental): a C++ API
  (`OmniEngine` / `OmniSession`, language + voice, thread count, backend `cpu`/`gpu`/`npu`) and a CLI, `omni_runner`.
  Built with **Bazel** + NDK r28+. No Kotlin/AAR for Kokoro documented yet.
- GPU through OpenCL (Adreno), WebGPU or Metal. Tested on Adreno and Pixel (PowerVR).

**Measured by Google** (18.3 s English paragraph, `af_heart`, 4 CPU threads, mean of 3):

| Phone | Backend | RTF | Peak memory |
|---|---|---|---|
| **Galaxy S26, Snapdragon 8 Elite Gen 5 (SM-S942U1)** | GPU (`gpu_fp16`) | **0.12** | 340–360 MB |
| Galaxy S26 | CPU (`int8_fp16`) | **0.26** | 915–930 MB |
| Pixel 10 Pro | GPU | 0.49–0.50 | 400–505 MB |
| Pixel 10 Pro | CPU | 0.93 | 925–945 MB |
| *Our S26 Ultra, sherpa 1.13.8 fp32 CPU (early)* | *CPU* | *0.47–0.63* | |

Engine load: 1.2–3 s on GPU (the first GPU run compiles kernels; a cache dir keeps them), ~0.5 s on CPU.

What this means for us:

- **The S26 Ultra could go from ~0.5 to ~0.12.** An hour of audio in ~7 minutes instead of ~30; and less memory.
- Even LiteRT's **CPU** path is ~2× faster than ours. Our int8 is slow because of ORT's dynamic quantization (see
  §4.3), not because of the chip.
- **Open questions (all UNVERIFIED):**
  1. **Text front end.** It uses misaki's lexicon + rules, **no espeak-ng**. It says nothing about unknown words.
     sherpa 2.0 failed exactly here (2.54 % words skipped). This has to pass the same S6 skipped-words check, or we
     feed it our own phonemes (the Omni API doesn't document phoneme input).
  2. **Licenses of the prebuilt GPU libraries.** `prebuilt/android_arm64/` has `libLiteRtGpuAccelerator.so`,
     `libLiteRtOpenClAccelerator.so`, `libLiteRtWebGpuAccelerator.so`, `libwebgpu_dawn.so`. The repo is Apache-2.0,
     but whether these binaries are open source is not stated. The older TFLite GPU delegate (OpenCL/GL) in the
     LiteRT repo is open source and is the fallback if needed.
  3. **Integration.** C++ only today. We'd write a small C shim and call it from the Rust adapter (or from Kotlin via
     a uniffi callback adapter). Bazel pulls dependencies from many hosts; some may be Google hosts we can't reach.
  4. **Audio cache.** A different engine gives different audio, so it needs its own engine id (as planned for any
     new adapter).
  5. Android 12+ apps must declare `<uses-native-library android:name="libOpenCL.so" android:required="false"/>`
     to load the vendor OpenCL library (from memory of the LiteRT GPU docs; UNVERIFIED today).

### 2.3 Other GPU routes

| Route | Finding |
|---|---|
| ExecuTorch | Software Mansion's `react-native-executorch` ships Kokoro split in 4 parts for **XNNPACK (CPU)**, fixed 32/64/128-token shapes. No GPU/Vulkan/QNN Kokoro build found, no published speed. |
| MNN / ncnn | **No Kokoro port found.** |
| Speech Central | Kokoro on an Adreno GPU via OpenCL with their own engine, RTF ≈ 0.93 on a 2020 mid-range phone (our Android options report). Closed app; proves GPUs work. |
| Soniqo | ORT on Android, RTF 0.58 measured on an **emulator**; their "2–3× with NNAPI" is a guess. Not useful. |

---

## 3. Exynos

- **S10+ (Exynos 9820) NPU: no.** Samsung's developer forum says its Neural SDK "is no longer available to third-party
  developers"; posts about the 9820's NPU say it isn't open to third-party programmers. NNAPI is the only door, and
  Kokoro doesn't fit NNAPI (S6 desk check; plus sherpa 1.13.8 refused NNAPI on the phone, S7).
- **Newer Exynos:** LiteRT supports the Samsung NPU ("Exynos AI LiteCore") only on **Exynos 2500 and 2600**
  (API 36), and ExecuTorch's Samsung backend lists the same two chips. Exynos 2400 isn't listed.
  Samsung's "Exynos AI Studio" toolchain exists; access for ordinary apps is UNVERIFIED.
- **S10+ GPU (Mali-G76 MP12):** LiteRT's GPU path supports OpenCL and OpenGL ES on Android, so it **may** run there.
  Kokoro on that GPU is untested by anyone (UNVERIFIED). Since the S10+ is red on CPU (RTF 1.2–1.8), it's worth one
  test with the same `omni_runner` binary.

---

## 4. CPU tuning

### 4.1 The two phones

| | S10+ (Exynos 9820) | S26 Ultra (SM8850 "for Galaxy") |
|---|---|---|
| Cores | 2 × Mongoose M4 (big) + 2 × A75 (middle) + 4 × A55 (little) | 2 × Oryon v3 prime (4.6 GHz) + 6 × Oryon v3 performance (3.62 GHz): **no little cores** |
| ISA features | Armv8.2: **dotprod, fp16** (from memory; check `/proc/cpuinfo` `asimddp`, `fphp`/`asimdhp`) | Armv9 with **SME1** (TechInsights); i8mm/SVE2 flags UNVERIFIED (check `/proc/cpuinfo`) |
| Best measured | fp32 **4 threads** (= the 4 big + middle cores); 8 threads much worse | 2–8 threads all 0.47–0.63; spread mostly heat |

### 4.2 Thread placement

- **big.LITTLE:** phones mix fast and slow cores. ORT splits work evenly across its threads, so one thread on a slow
  A55 core holds everyone back. That is why 8 threads were worst on the S10+.
- **ORT and affinity:** the ORT threading docs say that when the thread count is set explicitly (sherpa always sets
  it), **ORT sets no affinity**; the OS places the threads. ORT also has `session.intra_op_thread_affinities`
  (e.g. `"1,2;3,4"`) and `session.intra_op.allow_spinning`, but **sherpa-onnx's Rust API exposes neither** (only
  `num_threads` and `provider`).
- **What we can do without patching sherpa:** on Linux a new thread inherits its parent's CPU mask. So the Kotlin/Rust
  code that creates the engine runs on a dedicated "speech" thread, calls `sched_setaffinity(0, big cores)` (Rust
  `libc` crate) **before** `OfflineTts::create`, and ORT's worker threads (created then) inherit it. The same thread
  then runs `generate`. UNVERIFIED that ORT creates its pool at session creation in this build, so the test must check
  `/proc/<pid>/task/*/status` `Cpus_allowed_list`.
- **Android limits:** the app can only use cores in its **cpuset**. Background apps are put in a small set (mostly
  little cores). That is very likely why the S10+ fell to RTF 4–8 when the app went to the background (S7). So
  generation **must run in a foreground service** (already planned for M3).
- **Spinning:** ORT's worker threads spin (busy-wait) between ops by default. Faster, but it burns battery. Turning it
  off needs a sherpa patch or our own ORT session. Measure mAh before bothering.

### 4.3 Number formats: fp32, int8, fp16

- **Our int8 file is slow** (S10+: 45 % slower; S26: slower too) because it is **dynamically quantized**: every conv
  becomes `DynamicQuantizeLinear` + `ConvInteger`/`MatMulInteger`, and ORT's CPU kernels for `ConvInteger` are not
  well optimized (UNVERIFIED in detail, but it matches both phones). It isn't the chip: LiteRT's int8-weights model
  runs **faster** than our fp32 on the same S26.
- **dotprod / i8mm / SME** speed up real int8 math (statically quantized `QLinearConv`/QDQ in ORT, or XNNPACK in
  LiteRT). They don't help our current int8 file much.
- **fp16** halves memory traffic and doubles NEON width on Armv8.2+, but ORT's CPU EP has only partial fp16 kernels
  and Kokoro's vocoder is fp16-sensitive. Not worth it on ORT.
- **Practical rule for v1:** keep **fp32** on ORT; drop int8 from the "recommended" list on both phones; revisit
  formats only with LiteRT (where int8 weights + fp32/fp16 compute is the tested default).

### 4.4 Picking settings per phone at runtime

1. **Read the topology** (no permissions needed):
   `/sys/devices/system/cpu/cpu*/cpu_capacity` (relative speed, 0–1024) and
   `/sys/devices/system/cpu/cpufreq/policy*/cpuinfo_max_freq`, `related_cpus`; `/proc/cpuinfo` `Features`
   (`asimddp`, `i8mm`, `sve2`, `sme`, `fphp`). Keep only cores with capacity ≥ ~50 % of the fastest
   (UNVERIFIED threshold; S10+ → 4 cores, S26 Ultra → 8).
2. **Short self-benchmark on first run** (and after an app or OS update): one fixed built-in sentence (the voice-sample
   sentence, not book text), ~3 s of audio, at 2–3 thread counts within the big-core set, warm-up run first. ~10–20 s
   total. Keep the fastest; if two are within 5 %, keep the smaller count (less heat, less battery).
3. **Store** the result keyed by `Build.SOC_MODEL` (API 31+) + app version, in settings. Expose an "Engine threads:
   Auto" setting with a manual override.
4. Use the measured RTF to decide **"live" vs "Prepare-first"** (the S10+ class) on the first-run screen.

---

## 5. Recommendation (in order)

| # | What | Why | Effort |
|---|---|---|---|
| 1 | **CPU fixes in the current stack:** big-core affinity on the speech thread, thread count from §4.4, foreground service, fp32 only | Cheap, works on every phone, helps the S10+ most | 1–2 days (core + Android adapter) |
| 2 | **LiteRT desk test with `omni_runner`** on the S26 Ultra and the S10+: GPU vs CPU vs our sherpa numbers, battery, heat, ear test, skipped words | Google's numbers say ~4× on our exact chip; one CLI test tells us if it's real and if the front end is good enough | 2–4 days (most of it the Bazel build behind our blocked network) |
| 3 | If #2 wins: **a `LiteRtKokoro` adapter behind `SpeechEngine`** (C shim over Omni, or the raw `.tflite` graphs with our own front end), own engine id, CPU sherpa adapter stays as fallback | Clean plug-in, as the architecture intends | 2–4 weeks (front end + packaging + license review of the GPU `.so` files) |
| 4 | Only if battery is still bad: **QNN vocoder on the S26 Ultra** (cedgeremek's approach, recompiled for v81) | Possibly best energy; Snapdragon-only, closed libs | 3–6 weeks |
| — | Exynos NPU, NNAPI, ORT XNNPACK, MNN/ncnn | No path or no gain | drop |

---

## 6. Test plan for the S26 Ultra (small and concrete)

Rules: no book text anywhere (use a fixed paragraph we write ourselves, ~20 s of speech, stored in the test script);
results are ids, counts, timings; the phone is shared with another agent, so book a slot first.

### 6.1 What to build (laptop)

1. **Spike app, CPU tuning** (`lectrice-android/spike`, flavour `sherpa1`): add an `affinity` extra to the `engine`
   step (`--es cores big|all`) that pins the speech thread before creating the engine, and log
   `Cpus_allowed_list` of every thread of the process after creation. No new downloads.
2. **`omni_runner`** for `android_arm64` from `google-ai-edge/LiteRT-LM` at a fixed release tag (Bazel, NDK r28+:
   we have NDK 29). Fallback if Bazel can't fetch its dependencies here: ask before using any mirror; only with
   checksums.

### 6.2 Downloads (check before starting)

| File | From | Check |
|---|---|---|
| `kokoro_int8_gpu_fp16.litertlm` (295 MB) and `kokoro_int8_fp16.litertlm` (295 MB) | huggingface.co/litert-community/Kokoro-82M (pin a commit hash) | the SHA-256 that Hugging Face shows for each LFS file |
| LiteRT-LM source at a tag | github.com | git tag + commit hash |
| Bazel (the version in `.bazelversion`) and its deps | github.com releases (Bazelisk) | the release's sha256; Bazel deps are pinned by sha256 in the WORKSPACE/MODULE files |

Note: metered network, ~600 MB models plus Bazel deps (could be 1+ GB). No personal data in any request header.

### 6.3 What to run (via adb; `run-phone.sh` for the sherpa part)

| Run | How | Plugged in? |
|---|---|---|
| A. sherpa fp32 baseline, 4/6/8 threads, `cores=all` | existing `spike/run-phone.sh <serial> <large> <medium> 4,6,8` (already planned S5) | yes |
| B. sherpa fp32, 4/6/8 threads, `cores=big` (same as all on the S26; mostly to prove the mechanism) | `am start … --es step engine --es bundle fp32 --ei threads N --es cores big`, then `bench` | yes |
| C. LiteRT GPU (`int8_gpu_fp16`), and LiteRT CPU (`int8_fp16`) at 4 and 6 threads | `adb push` to `/data/local/tmp/omni`, run `omni_runner --model_name=kokoro --backend=gpu|cpu --voice=af_heart --text=<our paragraph> --cache_dir … --output_path …` 5 times each, first run counted separately (GPU kernel compile) | yes |
| D. 10-minute soak + battery, the best of A/B and the best of C | spike app "4b Soak when unplugged" for sherpa; a shell loop of `omni_runner` for LiteRT (over wireless adb) | **unplugged**, screen on, phone flat and cool |

Note: `omni_runner` runs as the shell user, not as an app, so the OS may schedule it differently. If C wins, confirm
inside an app before deciding.

### 6.4 What to measure

| Measure | How | Pass mark |
|---|---|---|
| RTF (warm median, and first run) | synthesis time ÷ audio duration (from the WAV length) | LiteRT GPU ≤ 0.25 to be worth an adapter |
| First sound | time to the first chunk (Omni streams) | ≤ 1 s |
| mAh per audio minute | `/sys/class/power_supply/battery/charge_counter` (µAh) before/after, unplugged, ÷ audio minutes | clearly below sherpa's (S10+ was ~45 mAh/min on CPU) |
| Heat | `dumpsys thermalservice` status + battery temperature every 30 s; RTF at minute 1 vs minute 10 | no drift > 20 %, status ≤ MODERATE |
| Memory | `dumpsys meminfo` total PSS (sherpa); `/proc/<pid>/status` VmHWM (runner) | ≤ 1.0 GB |
| Output | duration within ±3 % of sherpa's for the same text; user ear test on 3 clips (sherpa / LiteRT CPU / LiteRT GPU); **skipped-words check** (S6 method) on LiteRT's front end with our word lists | user doesn't prefer sherpa clearly; skipped words ≤ 0.1 % |
| Affinity | `Cpus_allowed_list` of every speech thread | matches the chosen cores |

Same steps on the S10+ for run C (GPU only), because that phone needs help most.

---

## Sources

- ONNX Runtime QNN EP: https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html
- ONNX Runtime threading: https://onnxruntime.ai/docs/performance/tune-performance/threading.html
- `onnxruntime-android-qnn` on Maven Central: https://central.sonatype.com/artifact/com.microsoft.onnxruntime/onnxruntime-android-qnn
- `qnn-runtime` on Maven Central (license = "Qualcomm AI Hub Model License"): https://central.sonatype.com/artifact/com.qualcomm.qti/qnn-runtime
- react-native-sherpa-onnx, QNN notes: https://www.mintlify.com/xdcobra/react-native-sherpa-onnx/guides/execution-providers
- sherpa-onnx releases: https://github.com/k2-fsa/sherpa-onnx/releases
- Kokoro on Hexagon (Android, S24 Ultra): https://github.com/cedgeremek/kokoro-offline-tts-android
- Kokoro on Hexagon DSP kernels: https://github.com/MansfieldPlumbing/Kokoro-Hexagon
- Qualcomm MeloTTS: https://huggingface.co/qualcomm/MeloTTS-EN, https://aihub.qualcomm.com/models/melotts_en
- RunAnywhere QHexRT (Hexagon bundles, Kokoro not there): https://huggingface.co/runanywhere
- LiteRT Kokoro: https://huggingface.co/litert-community/Kokoro-82M
- LiteRT-LM and Omni: https://github.com/google-ai-edge/LiteRT-LM (folder `omni`, `prebuilt/android_arm64`)
- LiteRT (vendors, Qualcomm/Samsung NPU): https://github.com/google-ai-edge/LiteRT, `litert/vendors/qualcomm`
- LiteRT NPU deployment via Play: https://developers.googleblog.com/unlocking-peak-performance-on-qualcomm-npu-with-litert/
- LiteRT Samsung / Qualcomm pages (search snippets; 403 to fetch): https://developers.google.com/edge/litert/next/samsung, https://developers.google.com/edge/litert/next/qualcomm
- ExecuTorch Kokoro: https://huggingface.co/software-mansion/react-native-executorch-kokoro
- ExecuTorch Samsung backend: https://docs.pytorch.org/executorch/main/android-samsung-exynos.html
- Samsung forum on NPU access: https://forum.developer.samsung.com/t/any-plans-for-a-third-party-npu-sdk-or-nnapi-support/23940, https://forum.developer.samsung.com/t/is-there-any-way-to-use-the-npu-inside-the-exynos-9820/11739
- Snapdragon 8 Elite Gen 5 CPU (SME1): https://www.techinsights.com/ko/node/61041; cores: https://www.guru3d.com/story/qualcomm-launches-snapdragon-8-elite-gen-5-with-oryon-gen-3-cpu-benchmarks-and-specs/
- Soniqo Android benchmarks (emulator): https://soniqo.audio/benchmarks/android
- Earlier project work: spec §15, `research/2026-10-24-sherpa2-oov.md`, `../lectrice-android/docs/progress/S7.md`, `research/2026-10-23-android-options.md` (Speech Central)

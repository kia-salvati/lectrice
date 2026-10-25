# Our own smaller Kokoro files for sherpa-onnx (laptop measurements)

2026-10-25. Laptop only (no phone touched), nothing committed. Every sentence used here was invented for this test
(no book text). Kokoro is Apache-2.0, so we may make and ship modified files (with a notice; see §6).

## 0. The short version

- **The official int8 file is slow because of how it was made, as the earlier research said.** It is
  "dynamic" int8: 90 `ConvInteger` + 83 `MatMulInteger` + 98 `DynamicQuantizeLinear` + 6 `DynamicQuantizeLSTM`.
  On the laptop it is **2.9× slower than fp32** (RTF 0.75 against 0.26), and it also sounds the most different from
  fp32 of everything tested.
- **Where the time goes:** 91 % of the time is the vocoder (the part that turns sound features into the waveform,
  `/decoder/generator`): its `Conv` layers (54 %) and its `Sin`-based activations (17 %). Shrinking the text part
  (BERT, LSTMs, MatMul) can't speed anything up, and quantizing it **breaks the timing** (sentences up to 60 % too
  long or short).
- **Two new files, both 111 MB (the official int8 is 114 MB):**
  - **"small" (`w8`)**: sounds like fp32, runs at fp32 speed, uses 140 MB less memory. It should replace the
    official int8 as our "small download" choice.
  - **"fast" (`q8`)**: 39 vocoder layers really run in int8. **19 % faster than fp32 on the laptop**, a little
    further from fp32 (needs the ear test). Phones may gain more (ARM has int8 dot-product instructions).
- **fp16 isn't worth it on sherpa's ONNX Runtime**: the general converter makes a broken file, and sherpa's Android
  ONNX Runtime has no fp16 Conv kernels, so it would just convert back to fp32.
- Both files load in sherpa-onnx 1.13.8 through our core. The phone test is ready (§5).

## 1. Setup

- sherpa-onnx **1.13.8 bundles ONNX Runtime 1.28.2** (version string in both the x64 static lib and the Android
  `libonnxruntime.so`). Our files keep the original format (ONNX IR 7, opset 14), so they load there.
- Tools: the ORT venv (`onnxruntime` 1.31, `onnx` 1.23). Nothing downloaded: fp16 used ORT's own
  `onnxruntime.transformers.float16`; `onnxconverter-common` and `sympy` aren't installed, and weren't needed.
- **Speed and loading** were measured through **our core** (`lectrice-infra::KokoroSpeechEngine`, the real sherpa
  1.13.8 static build), with a scratch binary outside the repo (the core's code was not changed). Laptop: Ryzen 9
  6900HX, **4 threads**, voice `af_heart`, 10 invented sentences (51 s of speech), one warm-up first, 3 rounds
  interleaved across all files. Another job was running on the laptop (load average 4–5), the same for every file.
- **Quality** compared with fp32 on the same 10 sentences:
  - **Duration**: the length of each clip against fp32 (limit ±3 %).
  - **MCD** ("mel-cepstral distortion", in dB): how different the sound's spectrum is from fp32, after lining the
    two clips up in time. Lower is closer. **Kokoro adds a little random noise on every run**, so even fp32 against
    itself scores about **3–3.6**. That is the floor. Rough guide: under ~5 = practically the same, 6–8 = small
    differences (needs the ear), 20+ = clearly different.
  - **Clipping**: samples at full scale (none anywhere) and broken values (none).
- Calibration (for the static int8): 80 invented sentences, half `af_heart`, half 7 other voices, turned into tokens
  with the bundle's own US lexicon.

## 2. Results

| File | Size | Loads in sherpa 1.13.8? | Laptop RTF (4 threads) | Memory (peak) | Duration vs fp32 (worst) | MCD vs fp32 (mean) | Verdict |
|---|---:|---|---:|---:|---:|---:|---|
| fp32 (official `model.onnx`) | 326 MB | yes | **0.261** | 600 MB | 0.08 % (run to run) | 3.6 (floor) | reference |
| int8 (official `model.int8.onnx`) | 114 MB | yes | 0.755 (2.9× slower) | 414 MB | 1.29 % | **28.2** | replace |
| **"small" `w8`** (vocoder weights int8, rest fp16, fp32 math) | **111 MB** | **yes** | 0.266 (= fp32) | **463 MB** | 0.38 % | **4.3** | **ship** |
| **"fast" `q8`** (39 vocoder convs real int8, rest as "small") | **111 MB** | **yes** | **0.211 (19 % faster)** | **451 MB** | 0.38 % | **7.7** | **phone test** |
| `w16` (all weights stored fp16, fp32 math) | 164 MB | yes | 0.258 | 647 MB | 0.36 % | 4.9 | beaten by "small" |
| `q8` with fp16 storage (`q8v_w16`) | 150 MB | yes | 0.207 | 579 MB | 0.37 % | 7.0 | beaten by "fast" |
| static int8, whole vocoder (`qdq_decsafe`) | 176 MB | yes | 0.190 (27 % faster) | – | 0.09 % | 20.4 | too different |
| static int8, whole model (task item a, first try) | 126 MB | yes | ~0.17–0.19 | – | **59–60 %** | 36–49 | broken timing |
| dynamic int8, MatMul only (task item b) | 289 MB | yes | 0.232 | – | **52 %** | 30 | broken timing |
| fp16 full, ORT converter (task item c) | 185 MB | **no**: invalid graph (types inside the `If` subgraph) | – | – | – | – | broken file |
| fp16 only in the vocoder convs ("islands") | 297 MB | yes | 0.258 (no gain) | – | 0.13 % | 3.3 | no gain |

Second voice check (`bf_emma`, same 10 sentences): fp32 floor 3.8, "small" 4.5, "fast" 6.1. Durations ≤ 0.2 %.

Where the rejected ones go wrong:

- **Durations come from the text half of the model** (BERT, the LSTMs, the duration predictor). Any int8 there
  (static or dynamic) changes how long each sound lasts. So only the vocoder may be quantized for real. The official
  int8 file gets away with 1.3 % only because it's dynamic (it measures each input on the fly), which is exactly
  what makes it slow.
- **Inside the vocoder, the last three resblocks are sensitive** (they work at the full 4,800 frames/s rate). Per
  block, quantizing alone (MCD; floor 3.0): resblocks 0–2: 3.3–4.6, noise_res 0–1: 3.0, resblocks 3–5: 9–16.
  Within those, the second conv (`convs2`, right after the Snake activation, feeding the residual) causes most of it.
  The "fast" file quantizes everything cheap (`noise_res.*`, `resblocks.0–2`) plus `convs1` of `resblocks.3–5`, with
  activation ranges set at the 0.01 %/99.99 % percentiles for resblocks 3–5 (cuts their error in half compared with
  plain min/max), and leaves `convs2` of resblocks 3–5, the sine source, and the STFT/iSTFT in fp32.
- **fp16 on ONNX Runtime CPU:** on x86 there's no gain, as expected. More important: sherpa's Android
  `libonnxruntime.so` (1.28.2) contains no fp16 Conv kernels (only a few fp16 contrib ops), so ORT would insert
  casts back to fp32 there too. Storing weights as fp16 does work (half size, same speed, MCD 4.9), but "small"
  does better.
- **The x86 int8 isn't hiding a hardware problem:** the "fake" int8 (QDQ left unfused, fp32 math) scored the same
  as the real int8 kernels (MCD 20.1 against 20.1), so the error is the quantization itself, not x86 saturation.
  The Android ORT has `QLinearConv` for both u8 and s8 and an i8mm code path.

**A side finding for the core:** a broken model file **aborts the whole process** inside sherpa-onnx
("Rust cannot catch foreign exceptions"), so `KokoroSpeechEngine::load` can't return an error. The app must check
the SHA-256 of a downloaded model **before** loading it (planned anyway for downloads).

## 3. The two new bundles (ready, gitignored under `models/`)

Same layout as `models/kokoro-int8-multi-lang-v1_0/` (all its other files copied unchanged), plus `NOTICE`,
`SHA256SUMS` (`sha256sum -c` format, 378 files) and `manifest.json` (path, size, SHA-256 per file, plus a version hash,
like the spike's bundled-model manifest). The model is named **`model.int8.onnx`**, so it fits the core's int8 slot
and the spike's `int8` bundle without code changes. Each model also carries `lectrice_variant` and
`lectrice_source_model_sha256` in its ONNX metadata.

| Bundle | Model SHA-256 | Manifest version |
|---|---|---|
| `models/kokoro-lectrice-w8-multi-lang-v1_0/` ("small") | `9892dbe1bf94d5b20e1199cd00dc4c849c0adae27b0bc2f8c76c4189f58c7a7a` | `233594bbc1d507e3…` |
| `models/kokoro-lectrice-q8-multi-lang-v1_0/` ("fast") | `d12189cf0a4aa9ae0a147b1d6af13deeaa585923c630a7433ab0a3f1a2b91dbe` | `0f8c66831b960fdb…` |

Source fp32 model: `b40f62b166ac8164b0627ef48a0b358eda0985e272fb03ef5252e7206305da11`.

How they were made (scripts in `research/2026-10-25-kokoro-variants-tools/`; `gunzip calib_stats.json.gz` first to
rebuild "fast" exactly):

1. `prep.py`: ORT's `quant_pre_process` (basic graph clean-up, no symbolic shapes) → `prep.onnx`.
2. `calib_collect.py`: min/max and percentiles of every Conv/MatMul/Gemm input and output, one sentence at a time
   (ORT's own calibrator ran out of memory on this laptop).
3. "fast": `qbuild.py prep.onnx q8v.onnx '{"ops":["Conv"],"include":["noise_res","resblocks.[012]/","resblocks.[345]/convs1"],"range":{"resblocks.[345]/":"p4","default":"minmax"}}'`
   (QDQ, per-channel int8 weights, uint8 activations), then `weightonly.py q8v.onnx q8v_w8.onnx 'mix:^/decoder/'`.
4. "small": `weightonly.py prep.onnx w8dec_w16.onnx 'mix:^/decoder/'` (decoder weights per-channel int8 through
   `DequantizeLinear`, the other large weights fp16 through `Cast`).
5. `meta.py` adds the metadata. `bench/` is the scratch Rust binary (needs `SHERPA_ONNX_LIB_DIR` pointing at
   `backend/target/sherpa-onnx-prebuilt/…/lib`; `KV_INT8=1` uses the int8 slot), `quality.py` the comparison.

## 4. Ear test (gitignored WAVs, each about 56 s, the same 10 invented sentences)

```
xdg-open research/2026-10-25-kokoro-variants-ear-test/1-fp32-reference.wav
xdg-open research/2026-10-25-kokoro-variants-ear-test/2-int8-official-sherpa.wav
xdg-open research/2026-10-25-kokoro-variants-ear-test/3-small-w8dec-w16.wav
xdg-open research/2026-10-25-kokoro-variants-ear-test/4-fast-q8v-w8.wav
xdg-open research/2026-10-25-kokoro-variants-ear-test/5-half-w16.wav
xdg-open research/2026-10-25-kokoro-variants-ear-test/6-rejected-qdq-whole-decoder.wav
```

What to listen for: 3 should sound like 1. For 4, any hiss, buzz or roughness compared with 1 (especially on "s",
"f" and quiet endings). 2 (the official int8) and 6 show what MCD 20–28 sounds like.

## 5. Phone test (S26 Ultra and S10+): exact commands

**Laptop x86 speed isn't phone speed.** Expectation: "small" ≈ fp32 speed on the phone (maybe slightly slower: its
int8 weights are turned into fp32 on every run; that cost 2 % on the laptop) with ~25 % less memory; "fast" should
gain at least the laptop's 19 %, maybe more with the S10+'s dot-product and the S26 Ultra's i8mm int8 instructions.

**Trick: no app change.** The variant goes into the spike's **`int8` slot**, so one `bench` run measures fp32 and the
variant side by side in the same session. While a variant sits in the int8 slot, **don't use Play** (its cached
clips would be stored under the official int8's engine id); `bench` and `accelerators` don't use the cache.

Run from `lectrice-android/`, with the `sherpa1` spike installed (the normal or the bundled build), the phone
plugged in, screen on, the app in front. Book a slot with the other agent first.

```bash
# 1. Pick the phone and the variant
SER=R58M23PXZYM      # S10+; for the S26 Ultra use its serial from `adb devices`
V=q8                 # q8 ("fast") or w8 ("small")
THREADS=4            # S10+: 4 ; S26 Ultra: 4,6,8
PKG=app.lectrice.spike.sherpa1
ACT=$PKG/app.lectrice.spike.MainActivity
SRC=../lectrice/models/kokoro-lectrice-$V-multi-lang-v1_0
STAGE=/data/local/tmp/lectrice-variant
SLOT=/sdcard/Android/data/$PKG/files/models/kokoro-int8-multi-lang-v1_0
OUT=spike/build/phone-runs/$SER-variant-$V-$(date +%Y%m%d-%H%M%S); mkdir -p $OUT

# 2. Start the app first (a bundled build re-installs its own int8 bundle at start, so push after this)
adb -s $SER shell am force-stop $PKG; adb -s $SER logcat -c
adb -s $SER shell am start -n $ACT
#    wait for "start done":
adb -s $SER logcat -s LectriceSpike:I | grep -m1 -E 'start (done|failed)'

# 3. Push the variant into the int8 slot (same files as push-model.sh) and check its SHA-256 on the phone
adb -s $SER shell "rm -rf $STAGE && mkdir -p $STAGE"
for f in model.int8.onnx voices.bin tokens.txt lexicon-us-en.txt lexicon-gb-en.txt espeak-ng-data SHA256SUMS; do
  adb -s $SER push $SRC/$f $STAGE/
done
adb -s $SER shell "cd $STAGE && grep -E '  (model.int8.onnx|voices.bin|tokens.txt|lexicon-(us|gb)-en.txt|espeak-ng-data/.*)\$' SHA256SUMS | sha256sum -c -" | grep -vc ': OK$'   # must print 0
#    (if this phone's sha256sum has no -c: `adb -s $SER shell sha256sum $STAGE/model.int8.onnx` and compare with §3)
adb -s $SER shell "rm -rf $SLOT && mkdir -p $SLOT && cp -r $STAGE/. $SLOT/ && rm -rf $STAGE"

# 4. Load it (the engine step moves the pushed folder in), then bench fp32 and the variant side by side
adb -s $SER shell am start -n $ACT --es step engine --es bundle int8 --ei threads 4
adb -s $SER logcat -s LectriceSpike:I | grep -m1 -E 'engine (done|failed)'      # look for "moved in from the pushed folder"
adb -s $SER shell am start -n $ACT --es step bench --es threadList $THREADS
adb -s $SER logcat -s LectriceSpike:I | grep -m1 -E 'bench (done|failed)'
# 5. Ear-test clips + duration check vs the CPU (writes fp32-cpu.wav and int8-cpu.wav; xnnpack/nnapi just fall back)
adb -s $SER shell am start -n $ACT --es step accelerators --ei fp32Threads 4 --ei int8Threads 4
adb -s $SER logcat -s LectriceSpike:I | grep -m1 -E 'accelerators (done|failed)'
adb -s $SER pull /sdcard/Android/data/$PKG/files/results $OUT/
adb -s $SER shell dumpsys meminfo $PKG | grep -E 'TOTAL PSS'

# 6. Repeat 2–5 with V=w8. Afterwards put the official int8 back:
spike/push-model.sh $SER int8      # then one "engine int8" step to move it in (or reinstall the bundled APK)
```

Read the result: in the newest `results/*.json`, the `bench` rows with `"bundle": "INT8"` are the variant and
`"FP32"` the reference (`medianRtf`). The `accelerator` rows give `maxDurationDeviation` (≤ 0.03). Listen to
`results/wav/int8-cpu.wav` against `fp32-cpu.wav`.

Pass marks: "small": RTF within 10 % of fp32 and memory clearly lower; "fast": RTF clearly below fp32 (≥ 15 %),
durations within ±3 %, and the user doesn't hear a difference that bothers them.

## 6. Recommendation

1. **Replace the official int8 with "small" (`w8`)** as the small-download choice, on all platforms: same size,
   fp32 sound, fp32 speed (2.8× faster than the official int8 here), 140 MB less memory. Confirm on the phones (§5).
2. **Test "fast" (`q8`) on both phones and by ear.** If it wins on the phone and passes the ear test, it becomes the
   default for slow phones (the S10+ class, where every 20 % matters). Otherwise keep fp32/"small".
3. **Drop:** fp16 compute on ONNX Runtime, dynamic int8, and any int8 outside the vocoder.
4. **Before shipping (later, not done here):** give each file its own engine id and file name in the core
   (`KokoroVariant`; spec rule C11: cached clips must never mix between files), check the SHA-256 before loading
   (§2 side finding), and ship the `NOTICE` (Apache-2.0 §4: say the file was modified) with the download.
   The big speed jump is still LiteRT's GPU path (research `2026-10-25-android-npu-gpu.md`); these files are the
   cheap win inside the current stack.

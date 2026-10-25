# LiteRT Kokoro on the laptop: does it run, how it sounds, skipped words, licenses

2026-10-25, Linux laptop only (Ryzen 9 6900HX, 16 threads; Radeon 680M integrated GPU). The phones weren't touched.
Counts only: no book text appears here or in any file written for this test.

## Short answer

| Question | Answer |
|---|---|
| Does it run on Linux x86-64? | **Yes.** `omni_runner` from LiteRT-LM (commit `9a9a74f`, 2026-10-24) built with Bazel 7.6.1 and runs `kokoro_int8_fp16.litertlm` with `af_heart`. |
| Speed on the CPU, 4 threads | **LiteRT RTF 0.48, sherpa 1.13.8 (fp32) RTF 0.30.** On this laptop's CPU, LiteRT is about 1.6× **slower** than what we ship. |
| Speed on the laptop's GPU | LiteRT on the Radeon 680M (Vulkan, through WebGPU): **RTF 0.11**, about 2.7× faster than our CPU path. A bonus finding for the desktop app. |
| Skipped words (S6 rule: ≤ 0.1 % average, ≤ 0.25 % worst book) | **Pass.** 22 of 297,933 words (0.007 %); worst book 0.029 %. sherpa 2.0 failed this at 2.54 %. |
| But… | LiteRT never drops a word it doesn't know: it **guesses** it from the spelling. That happens to **2.7 %** of words (5.1 % in GoF). Whether the guesses sound OK is for the ear test. |
| Licenses | Model, runtime and everything built from source: Apache-2.0 / BSD / MIT / similar. **No GPL anywhere** (no espeak-ng). The only open point: the prebuilt Android GPU `.so` files have no license or source stated. |

## Listen (ear test)

Same 10 invented sentences (`sentences.txt`), voice `af_heart`. Three files per sentence: our engine, LiteRT on the CPU
and LiteRT on the laptop GPU (to hear whether the GPU's fp16 math adds noise).

```sh
xdg-open ~/Desktop/learning/lectrice/research/2026-10-25-litert-ear-test/s01-sherpa1.13.8.wav
xdg-open ~/Desktop/learning/lectrice/research/2026-10-25-litert-ear-test/s01-litert.wav
xdg-open ~/Desktop/learning/lectrice/research/2026-10-25-litert-ear-test/s01-litert-gpu.wav
```

Replace `s01` with `s02` … `s10` for the others. Worth listening to on purpose (front-end guesses, see below):

- `s01`: "kubectl" (guessed), "getUserId", "AWS", "GCP".
- `s02`: "PostgreSQL" (guessed, sounds like "post-gress-kl"), "p99", "250 ms".
- `s06`: "HTTPServer" (guessed as one word, "h-t-p-server").
- `s07`: "nginx" (guessed), "Node.js".
- `s09`: "timeoutMs", "config.yaml" (both guessed).
- `s10`: "naïve", "résumé" (guessed, accents not handled), "C#" (read as just "k").

What the numbers already say:

- LiteRT's clips are about **6 % longer** in total (88.2 s vs 83.4 s): it speaks a little slower.
- LiteRT is about **2 dB quieter** (RMS level). The app could normalise this.
- CPU and GPU LiteRT clips have the same length and level: no sign of fp16 noise, but listen to be sure.

## Speed (RTF = time to make the audio ÷ length of the audio; lower is faster)

All 10 sentences, `af_heart`, 4 threads, laptop on mains power.

| Engine | How measured | RTF | 1 h of speech takes |
|---|---|---:|---:|
| sherpa 1.13.8 fp32 (our adapter, `litert_compare ear`) | one engine, warm, each sentence, 2 runs | **0.30** | 18 min |
| LiteRT CPU, `kokoro_int8_fp16` | each sentence, own process, mean of 3 | 0.48 | 29 min |
| LiteRT CPU | all 10 sentences in one run | **0.48** | 29 min |
| LiteRT CPU, 8 threads | all 10 sentences in one run | 0.38 | 23 min |
| LiteRT GPU (Radeon 680M, Vulkan via WebGPU) | each sentence, own process | 0.11 | 7 min |
| LiteRT GPU | all 10 sentences in one run | **0.115** | 7 min |

- LiteRT's model has a **fixed size**: every piece of up to ~124 phonemes (~10 s of speech) costs the same time,
  ~2.5 s on this CPU (~0.58 s on the GPU), however full it is. So a short sentence wastes most of the run: one-piece
  sentences got RTF ≈ 0.33, two-piece sentences ≈ 0.55. Joining short sentences before synthesis would help.
- Peak memory on the CPU: **880 MB** (sherpa fp32 is in the same range). Engine load: 0.6 s.
- On phones Google measured the opposite picture (Galaxy S26 CPU RTF 0.26, faster than our 0.47–0.63), because its
  int8 kernels suit ARM chips. The laptop result doesn't predict the phones: the phone test is still needed.

## Skipped and guessed words (5 books, the core's real cleaner)

**Method.** Same text and same word count as the sherpa 2.0 check (`research/2026-10-24-sherpa2-oov.md`): the core's
pipeline (pdfium → `RuleBasedCleaner` v3 → sentencex), sentences in `playback_order` with "read code & tables aloud"
off, `spoken_text()`, NUL removed; a word is a token (split at spaces and `, . ! ? ; :`) with a letter or digit. The
word totals match the sherpa check exactly (297,933). The sentences were piped from the scratch tool straight into a
counter; nothing but counts was ever written.

The counter (`~/.local/src/LiteRT-LM/lectrice_tools/oov_count`, local only) runs **LiteRT's own front end**:
`KokoroPhonemizer`, set up from the `.litertlm` file exactly as the engine does it (`en-textnorm` rules +
`en-lexicon`), then the same steps through an instrumented copy of `english_g2p.cc` that counts which words fell back
to letter-to-sound rules. The copy gave the same phonemes as the engine for every one of the 23,434 sentences.

**What LiteRT does with a word**, from the source (`omni/text2speech/kokoro/english_g2p.cc`): text normaliser first
(numbers, dates, money, units), then the misaki lexicon (same source as sherpa 2.0's), then rules (inflections,
`-er`, British spellings, prefixes, short acronyms spelled out), then **letter-to-sound rules** that guess any word.
Only when even that gives nothing does it write an "unknown" mark (❓), which isn't in Kokoro's symbol set and is
silently dropped. So a *skip* is a ❓; a *guess* is a letter-to-sound fallback.

| Book | Words | Skipped | Skipped % | Guessed by spelling rules | Guessed % | Symbol-only tokens dropped (not words) |
|---|---:|---:|---:|---:|---:|---:|
| Clean Architecture | 75,232 | 3 | 0.004 % | 535 | 0.71 % | 36 |
| Designing Distributed Systems | 37,165 | 0 | 0 % | 443 | 1.19 % | 29 |
| GoF Design Patterns | 85,738 | 0 | 0 % | 4,357 | 5.08 % | 12 |
| LinkedIn Posts 2024 | 37,689 | 11 | 0.029 % | 581 | 1.54 % | 393 |
| The Kubernetes Book | 62,109 | 8 | 0.013 % | 2,171 | 3.50 % | 42 |
| **All books (pooled)** | 297,933 | **22** | **0.007 %** | 8,087 | **2.71 %** | 512 |
| Average of the 5 book shares | | | 0.009 % | | 2.40 % | |
| **Worst book** | | | **0.029 %** | | 5.08 % | |

- **Verdict on the S6 rule: pass**, with a wide margin (limits 0.1 % / 0.25 %).
- **The 22 skipped words** are all numbers glued to symbols: keycap-digit emoji (6), `~5`-style (4), exponents like
  `1^-9` (3), IP address with port (3), clock-like `1:2:3` / `1:2` (2), and one each of `2×3`, `5~6`, `1/2.`, `*5`.
  A small text rule in the adapter could fix most of them.
- **Symbol-only tokens** (a lone `-` used as a bullet: 371 of them, mostly in the LinkedIn book; `.`, `•`, `>`, `×`, `/`)
  also get the ❓ mark and are dropped. They aren't words, and silence is right for them, as with both sherpa versions.
- **Guessed words**, sorted the same way as the sherpa check (all-caps / with digits / camelCase / attached punctuation
  / other): 6 / 136 / 550 / 461 / 6,935. Almost all are names, product and tool names and, in GoF, words run together
  by the PDF's missing spaces. These are the same words sherpa 2.0 dropped (its leftover was 2.54 %), because the
  lexicon is the same. LiteRT guesses them instead of dropping them.
- **How good the guesses are** isn't measurable from counts. From the invented sentences: "DevOps" is fine,
  "PostgreSQL", "kubectl", "nginx", "HTTPServer", "résumé" and "C#" come out wrong or odd. sherpa 1.13.8 also guesses
  unknown words, but with espeak-ng's mature rules, so the ear test on `s01`, `s02`, `s06`, `s07`, `s09`, `s10` is the
  real comparison. Google's own number: 3.96 % word error rate for English on hard test sentences (model card).
- Nothing else was lost: 0 symbols outside Kokoro's set, 0 sentences that came out empty, 0 errors.

## Licenses

| Piece | License | Source checked |
|---|---|---|
| Kokoro weights (`litert-community/Kokoro-82M`) | Apache-2.0 (from `hexgrad/Kokoro-82M`) | model card |
| English lexicon inside the file | misaki, Apache-2.0 | model card |
| Japanese / Chinese data inside the file | unidic-lite BSD; jieba / pypinyin MIT | model card |
| LiteRT-LM (incl. Omni, Kokoro front end) | Apache-2.0 | repo `LICENSE` |
| Built from source into `omni_runner` | LiteRT, TensorFlow, XLA/TSL, abseil, flatbuffers, ruy, gemmlowp, sentencepiece, tokenizers-cpp, ml_drift (Google's GPU engine, incl. OpenCL/Vulkan/WebGPU code): Apache-2.0. XNNPACK, cpuinfo, pthreadpool, protobuf, re2, Dawn, kissfft: BSD. stb, SafeInt, FP16, FXdiv, cppjieba, farmhash, Oniguruma (via `onig_sys`): MIT. miniaudio: public domain / MIT-0. zlib, minizip: zlib. Eigen: MPL-2.0 (file-level copyleft; fine as long as we don't change Eigen files). fft2d (Ooura): permissive notice. Rust crates (tokenizers, regex, serde…): MIT/Apache-2.0. | `LICENSE`/`COPYING` of each Bazel repo actually linked (`bazel cquery deps(//omni:omni_runner)`) |
| Prebuilt `libwebgpu_dawn.so` (Linux, linked) | Dawn is BSD-3; the binary itself has no license file | LFS object, no notice |
| **Prebuilt Android GPU libs** (`libLiteRtGpuAccelerator.so`, `libLiteRtOpenClAccelerator.so`, `libLiteRtWebGpuAccelerator.so`, `libwebgpu_dawn.so`) | **Not stated.** No license file, no copyright strings, no build recipe in the repo. | `strings`, repo tree |
| GPL? | **None.** No espeak-ng, unlike sherpa 1.13.x. | |

On Linux the runner registered a **statically linked GPU accelerator** built from the open `ml_drift` source
(Apache-2.0), so the only prebuilt binary we used was Dawn. `ml_drift` also has OpenCL code, so an Android build from
source may avoid the unlabelled `.so` files. That needs checking during the phone work.

## Downloads (all checked against a checksum)

| What | From | Check |
|---|---|---|
| `kokoro_int8_fp16.litertlm` (295 MB) | huggingface.co (official) | sha256 `e7b931ee…7fa1` = Hugging Face's LFS id |
| LiteRT-LM source | github.com (official) | git |
| Prebuilt `.so` files (the clone had only Git LFS pointers; 9 files, linux_x86_64 + android_arm64) | GitHub LFS (`github-cloud.githubusercontent.com`, official) | sha256 of each file = id in its pointer |
| Bazel dependencies | storage.googleapis.com, github.com, gitlab.com, mirror.bazel.build, crates.io | sha256 pinned in LiteRT-LM's `WORKSPACE` / Cargo lock, checked by Bazel |
| **Mirror:** `ghfast.top` (public GitHub proxy) | set up by the earlier agent in `~/.cache/bazel-downloader.cfg` to be tried before github.com | Bazel checks the same sha256, so a mirror can't change the files |
| `litert_lm_api-0.18.1` wheel (47 MB) | files.pythonhosted.org | sha256 from PyPI; downloaded by the earlier agent, **not used** |

No personal data went into any request header (plain `curl`/Bazel; the LFS request sent only `Accept` and
`Content-Type`).

## What's left on disk

| Path | Size | Why keep it |
|---|---:|---|
| `~/.cache/bazel-repo` (repository cache) | 1.1 GB | All downloaded archives by checksum; an Android build reuses them without the network. |
| `~/.cache/bazel-distdir` | 115 MB | Archives fetched by hand when Bazel's download failed. |
| `~/.cache/bazel/…/external` (unpacked sources and host tools) | 6.5 GB | Same output base as an Android build, so it skips unpacking again. x86 build outputs were removed (`bazel clean`). |
| `~/.cache/bazel/install` | 182 MB | Bazel itself. |
| `~/.local/src/LiteRT-LM` | 170 MB | Checkout with the real LFS libs (Android ones included) and `lectrice_tools/` (the counter). |
| `~/.local/opt/litert-lm-omni/` | 68 MB | Ready `omni_runner` + `oov_count` + `libwebgpu_dawn.so` (run with `LD_LIBRARY_PATH` set to that folder). |
| `models/litert-kokoro-82m/kokoro_int8_fp16.litertlm` | 295 MB | The CPU/GPU model (gitignored). |

**Disk is tight: `/home` has 16 GB free.** If space is needed, `bazel clean --expunge` in the checkout frees the
6.5 GB; the Android build then unpacks again from the 1.1 GB cache (a few Python packages may need the network).

## What the phone GPU test needs

1. **Download the GPU model**: `kokoro_int8_gpu_fp16.litertlm`, 294,750,530 bytes, sha256
   `1ef178b127ec48c737daf17eabb4cea5f6b3ffbc510aced270fe5c267bbeac52` (metered network: ~295 MB). It uses half the
   memory of the file we have. The file we have also runs on the phone GPU, so the test can start with it.
2. **Build `omni_runner` for arm64**: `bazel build -c opt --config=android_arm64 //omni:omni_runner` with
   `ANDROID_NDK_HOME=~/Android/Sdk/ndk/29.0.14206865` and the same memory flags (`--jobs=3 --local_resources=…
   --host_jvm_args=-Xmx2g`). Google tested NDK r28c; r29 is untested. The x86 build peaked at about 2.9 GB (Bazel + compilers, logged);
   the Android build reuses the caches above.
3. **Push and run**: the runner, `prebuilt/android_arm64/*.so` (already fetched and checked) and the model to
   `/data/local/tmp`, then `LD_LIBRARY_PATH=… ./omni_runner --model_name=kokoro --backend=gpu --num_threads=4
   --voice=af_heart --text=… --output_path=…`, also with `--backend=cpu` for comparison. Use the same 10 sentences,
   with the phone cool and plugged in, as Google did.
4. **Phones**: S26 Ultra (Adreno, the chip Google measured). S10+ (Mali-G76): see whether OpenCL loads at all.
5. **Before shipping anything**: find out what the Android GPU `.so` files are (license), or build the GPU path from
   `ml_drift` source as the Linux build did; and decide what to do about the 2.7 % guessed words, after the ear test.

## Tools (local, not committed)

- `~/.local/src/LiteRT-LM/lectrice_tools/`: `oov_count.cc` (counter; `--show` prints IPA, used only on the invented
  sentences) and `english_g2p_counted.cc/.h` (copy of LiteRT's `english_g2p.cc`, Apache-2.0, with two counters).
- `litert_compare` (scratch Rust tool from the earlier agent): `ear` makes the sherpa clips and timings, `dump` pipes a
  book's spoken sentences to stdout. Binary: `backend/target/release/litert_compare`.
- Book loop used for the counts (prints counts only):
  `for b in ~/Desktop/*.pdf; do printf '\x1d%s\n' N; litert_compare dump <libpdfium.so> "$b"; done | oov_count --model_path=<model>`.

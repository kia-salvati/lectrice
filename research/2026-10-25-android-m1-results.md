# Android M1 spike: results and decision (2026-10-25)

The decision page for the go/no-go spike (`docs/2026-10-23-android-m1-spike-plan.md`, §6). Detailed reports:
`lectrice-android/docs/progress/S5.md` (Galaxy S26 Ultra) and `S7.md` (Galaxy S10+). Raw results (ids, counts, durations
only): `lectrice-android/docs/results/S26U-sherpa1-*.json` and `S10plus-sherpa1-*.json`.

## The decision: **GO**, with Prepare-first for older phones

- **GO:** the S26 Ultra (the go/no-go phone) is **green on sustained RTF (0.58, limit 0.6)** and passes every hard check
  (native load, memory, import).
- **The S10+ is red** (sustained 1.2–1.6). The S10+ sets the minimum phone class, so by §6 **phones like the S10+ get
  Prepare-first** (generate ahead, best while charging), and the app says so on its first run.
- This means **Prepare mode (F2) gets built**, but only for slower phones. Recent phones listen live with read-ahead,
  like the desktop.
- NO-GO doesn't apply: even the S10+ makes an hour of audio in about 1.2–1.8 h (RTF < 2).

**What you need to decide:** accept GO with Prepare-first for older phones (recommended), or ask for the follow-ups below
first.

## Per phone

| Check (plan §6) | Galaxy S26 Ultra (SM8850, Android 17, unplugged) | Galaxy S10+ (Exynos 9820, Android 12) |
|---|---|---|
| Native load | green | green |
| **Sustained RTF** (soak minutes 8–10, fp32) | **0.583 green** (8 threads) | **1.22–1.58 red** (4 threads, unplugged) |
| Best bench RTF, fp32 | 0.465 (8 threads), 0.478 (2 threads) | 1.76 (4 threads, plugged) |
| Best bench RTF, int8 | 0.719 (2 threads) | 2.44 (4 threads) |
| First sentence (press → sound) | 2.3–3.0 s, amber | 4.7–14.9 s, red |
| Engine load, warm | 0.7–1.9 s, green | 2.4–7.6 s, amber |
| Peak memory | PSS 812 MB, green; VmHWM 1.04 GB over a long session, amber | PSS 927 MB; VmHWM 0.84 GB in the bench, 1.44 GB in a long session |
| Heat (10 min) | **SEVERE after 85 s, RTF doubled** (0.29 → 0.62): red on the drop | ≤ LIGHT, stable: green |
| Battery | **15.9 mAh per audio-minute**; 7 % for 21.7 audio min (≈ 19 % per audio hour) | 45 mAh per audio-minute (inflated by throttling) |
| Import of the largest book | 0.44 s, green | 4.2 s, green |
| `get_document` | 5–14 ms, green | 39–194 ms, green |
| Gaps while playing | medium book: none, green; large book (very short sentences, hot phone): 10 rebuffers in 2 min | regular buffering, red |
| APK (arm64 release, no models) | 40.8 MB, green | same |

Notes on the S26:

- **Sustained 0.58 is enough for live listening:**
  - 1×: RTF ≤ 1 is enough, so there is margin.
  - 1.5×: needs ≤ 0.67, so it just works.
  - 2×: needs ≤ 0.5, so live mode can't sustain it. At 2×, Prepare mode or a pre-filled read-ahead covers the gap.
- **Heat is the real limit.** At 8 threads the phone reaches SEVERE in under 2 minutes. Ways to handle it:
  - Fewer threads: 2 threads were as fast as 8 when the phone was cool.
  - Pausing generation once the read-ahead is 10 minutes ahead (at 1× the engine only needs to run about 60 % of the
    time).
- **The large-book buffering is a product issue, not a speed limit:**
  - The player's own preloading and the core's read-ahead fight over the one engine.
  - Very short sentences each carry a fixed cost.
  - Fix in M3: one generation queue the player waits on, and short sentences merged into batches before they reach the
    engine.
  - The S10+ showed the same pattern.
- **Background throttling** (S7) still applies on Samsung: real generation must run in a foreground service.

## Threads per phone

| Phone | fp32 | int8 |
|---|---|---|
| S26 Ultra | 2 ≈ 8 when cool, 8 fastest; **default 4 proposed** to limit heat (to confirm with 2t/4t soaks) | 2 |
| S10+ | 4 (more doesn't help; 8 is much worse) | 4 |

The app should keep a per-phone default (cores and SoC class), and Settings can override it.

## Model choice notes for the first-run screen (spec §6.1, draft)

- **Full quality (fp32) · about 370 MB:** "Faster than real time on recent phones; on older phones, prepare chapters
  ahead."
- **Smaller (int8) · about 126 MB:** "Smaller, but slower on every phone we tested; may not keep up on older phones."
- "Not sure? Full quality." Here fp32 is both the better voice and the faster one on both phones. int8 is 50–100 % slower
  on the S26 and about 45 % slower on the S10+.

## sherpa-onnx 2.0

**Rejected** (S6, `research/2026-10-24-sherpa2-oov.md`): it skips 2.54 % of spoken words after the simple fixes, against
a limit of 0.1 %. The core stays on 1.13.8, so the APK carries GPL-3 espeak-ng (spec §14.3). The speed comparison (M-l)
is moot.

## Accelerators

- **xnnpack: drop.** Not in sherpa 1.13.8's Android onnxruntime ("Available providers: Nnapi, CPU … Fallback to cpu!").
  RTF was the same as `cpu` on both phones.
- **nnapi: drop.** sherpa refuses it on both phones ("requires API level >= 27. Current API level 21"): it checks the API
  level its libraries were built for, not the phone's. NNAPI is also deprecated since Android 15, and the desk check
  (M-o) found NNAPI could take only about half the graph, split into 64–98 pieces, which ORT says is slower than the
  CPU.
- Battery per audio-minute was only measured on `cpu`, since no accelerator engaged.
- The GPU and the Snapdragon NPU are being researched separately (`research/2026-10-25-android-npu-gpu.md`). Given the
  S26's heat behaviour, they matter more for battery and heat than for speed.

## What changes vs the spec

1. **Prepare mode (spec §8.5) is in**, aimed at older phones. The first run says so on S10+-class phones.
2. **Generation runs in a foreground service** (Samsung throttles background work 4–8×).
3. **Playback and read-ahead use one generation queue** (no second engine call from the player). Short sentences are
   merged into batches.
4. **Read-ahead pauses** once it is far enough ahead (heat and battery), and the thread count is a per-phone default.
5. sherpa-onnx stays on 1.13.8 (GPL-3 note in the APK).
6. The bundled-models APK (539 MB) is for testing only. Shipping stays with the model download (spec §6).

## Follow-ups (optional, before M3 sets the defaults)

- S26: 10-minute soaks at 2 and 4 threads (cool phone, battery ≥ 60 %, unplugged), to pick the thread default by
  sustained RTF and mAh per audio-minute.
- Cold engine load on both phones (after a reboot; needs the PIN typed in).
- The S10+ soak was partly spoiled by background throttling; a clean re-run would give its true battery cost.

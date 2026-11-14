# 24 · Desktop helper ("your computer makes the audio")

Status: **decided by Opus 2026-11-13, for the user's review** (the user asked for the feature on 2026-11-13 and said to
build it as recommended without waiting; every open point in §8 has a recommended answer that is followed unless the user
overrules it). Nothing here is built. Desktop side: `lectrice-desktop/docs/2026-11-13-phone-helper-spec.md`.
UX notes for the later design brief: §11. Numbers marked **UNVERIFIED** are checked in milestone H0 (§12).

This feature replaces the old rule "no server option for the phone" (user 2026-10-23) **for this opt-in mode only**
(user 2026-11-13). The phone never needs the computer: everything still works on the phone alone.

Change ids: **C40–C52** (C35–C39 are left free for file 23, which is being written at the same time).

## 1. Goal

When the phone and a paired Lectrice desktop app are on the same home Wi-Fi, the **desktop makes the sentence audio** (it
is 3–10× faster than a phone and has no battery or heat limit) and sends it to the phone, which plays it exactly as if it
had made it itself. Read-ahead in this mode is **30 listening-minutes**, so a weak or dropped link is never heard: when the
link goes, the phone keeps playing what it has and **falls back to making audio itself** (or to prepared / audiobook audio)
with no gap. Nothing ever leaves the home network; there is no cloud, no account, no relay.

## 2. User flow

**Pairing (once per phone + computer):**

1. Desktop: Settings → **Phone** tab → turn on **"Let my phone use this computer"**. (Windows: the first time, Windows
   asks whether Lectrice may use private networks; the tab explains this first. Desktop spec §5.)
2. Desktop: **"Pair a phone"** → a card shows a **6-digit code** (valid 2 minutes) and the computer's name.
3. Phone: Settings → **Computer helper** → **"Pair with a computer"** → Android asks for **Nearby devices** (local network)
   permission (only when the app targets SDK 37+, §3.10) → a list of Lectrice computers on this Wi-Fi → tap one → type
   the code.
4. Both screens: "Paired with *Kia's laptop*" / "Paired with *Galaxy S10+*". Done. The phone remembers the computer; the
   computer remembers the phone.
5. If the list stays empty (some routers block discovery): **"Enter the address instead"** → the IP and port the desktop
   card shows (e.g. `192.168.1.20:47878`).

**Listening (every day, no steps):**

1. Press play at home. If the paired computer is reachable and has Lectrice open with the helper on, the pill's status
   line shows a small `computer` mark and the player sheet says **"Making audio on Kia's laptop"**. The slider's buffered
   part grows to ~30 minutes ahead within a few minutes.
2. Walk out of Wi-Fi range, the computer sleeps, or Lectrice on the computer quits: playback continues from the 30
   buffered minutes. The sheet line changes to **"Computer not reachable · 24 min ready"**; the phone takes over making
   audio when it needs to (§3.6). One quiet snackbar the first time per session: "Your computer isn't reachable. The
   phone carries on by itself."
3. Back in range: it reconnects by itself and takes over again. No sound changes, no prompt.

**Turning it off:** phone Settings → Computer helper → switch off (keeps the pairing) or "Forget this computer";
desktop Phone tab → switch off, or "Forget" next to a phone.

## 3. Rules & logic

### 3.1 The shape of the solution (one picture)

```
phone (Kotlin)                         phone core (Rust, lectrice-ffi)                       desktop (Tauri, Rust)
NSD discovery, permission,  ──addr──►  SpeakSentence ──miss──► local SpeechWorker (engine)
Wi-Fi network, governor,               ReadAhead (10 min, local)                              ┌─────────────────────┐
status UI                               HelperFill (30 min) ──items──► SpeechHelper port ─────┤ lectrice-link server│
                                              ▲                       (LinkSpeechHelper,      │ ServeSpeechHelper   │
                                              └──clips──► AudioCache   Noise over TCP, LAN)   │ → its SpeechEngine  │
                                                         (same key, same WAV)                 └─────────────────────┘
```

- The desktop is **a producer of sentence clips**, not a new player source. Its clips go into the phone's normal audio
  cache under the phone's normal `AudioKey`, so the player, the slider, Prepare and export all work unchanged; they just
  find more clips already made.
- The phone's **own engine stays the fallback** and the only engine the phone's player relies on. The helper is never
  required: any failure on its side ends in "the phone makes it" (§3.6).
- The protocol, crypto and both ends live in **one shared core crate** (`lectrice-link`, C40) so the phone and desktop
  can never disagree on the wire format, and the whole link is tested end to end in the main repo (§13).

### 3.2 Discovery and pairing

**Recommendation: mDNS discovery + a 6-digit code typed on the phone, checked with a PAKE (SPAKE2).** QR is a later
convenience (§8), not v1.

| Option | For | Against | Verdict |
|---|---|---|---|
| **6-digit code + SPAKE2** (desktop shows, phone types) | no camera permission, no scanner library; the code is never sent, so a listener on the Wi-Fi learns nothing and can't brute-force it offline; 3 tries per code → an online guesser wins with p = 3 × 10⁻⁶ | typing 6 digits once | **v1** |
| QR code (desktop shows, phone scans; carries address + desktop key fingerprint + a 128-bit secret) | no typing; no PAKE needed (the secret is long) | camera permission + a scanning library (CameraX + zxing-cpp, or Google's ML Kit/Play Services scanner, which we avoid); a webcam-less laptop shows it fine but the phone must aim | later (H-later), as an extra path to the same pairing |
| Numeric comparison (both screens show 6 digits from the handshake; the user checks they match) | nothing to type | without a commitment step a man-in-the-middle can grind ephemeral keys until both sides show the same 6 digits (microseconds for 10⁶ tries); doing it right means a Bluetooth-style commitment protocol of our own | no |
| Code mixed into Noise as a pre-shared key (`XXpsk`) without a PAKE | one library | a 6-digit PSK can be brute-forced offline from one recorded handshake | no |

**Discovery.** The desktop advertises, only while its helper is on, the DNS-SD service **`_lectrice._tcp`** on its
listening port (default **47878**, any free port if taken) with TXT records: `v=1` (protocol major), `id=<first 16 hex of
SHA-256(desktop static public key)>` (no `pair=` record: it would tell an attacker when to guess, security S5). The instance name is the
computer name the user sees in the Phone tab (editable). The phone browses with **`NsdManager`** (Kotlin) and identifies
a known computer by `id`, never by name or IP (both change).

**Pairing protocol** (C41; all in `lectrice-link`, test vectors in §13):

1. Phone → desktop TCP connect; plaintext prologue `lectrice-link/1 pair` (bound into both handshakes below).
2. **SPAKE2** (Ed25519 group, symmetric identities `"lectrice-phone"` / `"lectrice-desktop"`, password = the 6 digits) →
   32-byte key `K`.
3. **Noise `XXpsk3_25519_ChaChaPoly_BLAKE2s`** with `psk = HKDF(K, "lectrice-pair-v1")`: both sides send their **static
   X25519 public keys** encrypted and authenticated by the code. A wrong code fails here (both sides see a MAC error).
4. Inside the now-encrypted channel: `PairHello { device_name, app_version, proto_minor }` both ways.
5. Each side **pins** the other's static key (phone: one computer per entry, up to 4; desktop: up to 8 phones). The
   desktop UI shows "Paired with *name*", the code card closes.

Limits: the code lives **2 minutes** and allows **3 failed attempts**, then it's replaced (the card says "A wrong code
was typed…"); **changed by security S4:** the 3 wrong codes count per window across all addresses and the 3rd
**closes** the window (no automatic new code: the computer's user clicks "Pair a phone" again; ≤ 10 failures per hour); at most one pairing at a time; the pairing window opens **only** when the desktop user
clicks "Pair a phone". Re-pairing a known phone replaces its key.

### 3.3 Sessions: encryption and authentication

- Every later connection is **Noise `IK_25519_ChaChaPoly_BLAKE2s`** (via the `snow` crate): the phone already knows the
  desktop's static key (pinned), so the handshake is one round trip, the phone's identity is encrypted, and the desktop
  accepts the session **only if the phone's static key is in its paired list**. Prologue `lectrice-link/1 session`.
- **No plaintext ever** beyond the 1-line prologue; no fallback to an unencrypted mode; no "trust on first use" outside
  pairing.
- Transport framing: `u16 length ‖ Noise ciphertext` (Noise caps a message at 65 535 bytes); clips are split into
  ≤ 60 KB chunks. Plaintext messages are `postcard`-encoded enums (§3.5). Nonces are Noise's counters, so replays and
  re-ordering break the session.
- **LAN only:** the phone connects only over its current **Wi-Fi or Ethernet network** (never mobile data; socket bound
  to that `Network`, §3.10) and only to private / link-local addresses (IPv4 10/8, 172.16/12, 192.168/16, 169.254/16;
  IPv6 fc00::/7, fe80::/10). The desktop drops any peer outside those ranges (also CGNAT 100.64/10, so VPN overlays such
  as Tailscale don't silently turn it into "over the internet"). No relay, no cloud, no port forwarding.
- Keys: X25519 static key pair per install. Phone: private key in `filesDir/lectrice/link/identity.key`, encrypted with
  an AES-GCM key from the **Android Keystore** (non-exportable), excluded from Auto Backup (file 17). Desktop: §4 of the
  desktop spec (file `0600` in the app data dir). Forgetting a device deletes its pinned key on that side; the other side
  gets "not paired" on its next connect and shows "Pair again".

### 3.4 What is sent (privacy and bandwidth)

**Recommendation: send only the speech text of the sentences the phone wants, with their sentence ids and the voice.**
No document id, no title, no PDF, no cleaned document, no position, no chapter names.

| Option | Data on the desktop | Bandwidth | Verdict |
|---|---|---|---|
| **Sentence items** `{ item_id = the app's own counter (H2: never the sentence id), voice_id, text = sentence.spoken_text() }`, ≤ 30 listening-minutes ahead | only in RAM, only until that clip is sent; never written, never logged | text ≈ 18 B per second of audio → **~32 KB per 30 min**, negligible | **v1** |
| The whole document (JSON, or the PDF) once, then sentence ids | the book stored on the desktop (a second copy of copyrighted text at rest; must match cleaner versions) | 1–5 MB once | no: more data at rest for no gain, since the phone already plans |

- The text is the exact `spoken_text()` that `SpeakSentence` would give the local engine (cues "Code example." / "Table."
  included; empty-speech sentences are never sent, they become local silence as today).
- The desktop **doesn't cache** phone clips on disk and doesn't add anything to its library. Its logs carry only counts,
  item ids, durations, engine id and errors (README §4.6 applies on both sides).
- The phone's `AudioKey` (document id, cleaner version, engine id, voice, sentence id) never leaves the phone; the
  desktop sees only `item_id` (the app's own counter, meaningless without the book; H2).

### 3.5 The wire protocol (v1, C40)

Messages after the handshake (phone = P, desktop = D):

| Dir | Message | Fields | Notes |
|---|---|---|---|
| P→D | `Hello` | `proto_minor`, `app_version`, `device_name` | D answers `Welcome { proto_minor, app_version, computer_name, engines: [EngineOffer] }` (`EngineOffer { engine_id, revision, installed, sample_rate }`) |
| P→D | `Open` | `engine_id`, `revision`, `voice_ids` | D loads that engine if installed (desktop spec §3) → `Ready { engine_id, revision, sample_rate, voices }`, or `Refused { error }` (`HelperModelMissing` + `bundle_id`, `HelperIncompatible`) |
| P→D | `Make` | `batch_id`, `items: [{ item_id: u32, voice_id, text, urgent: bool }]` (≤ 32 items, text ≤ 4 KB each) | D queues them; `urgent` items go first (§3.7) |
| P→D | `Cancel` | `batch_id` or `all` | queued items dropped; the one being made finishes and is discarded |
| D→P | `Clip` | `item_id`, `sample_rate`, `samples: u32`, `make_ms`, chunked FLAC bytes (`seq`, `last`) | §3.8; one clip at a time per session, in finish order |
| D→P | `ItemFailed` | `item_id`, `error` (registry name + code) | P makes that sentence locally |
| both | `Ping` / `Pong` | `t_ms` | every 5 s while idle |
| D→P | `Load` | `queue_items`, `own_listener: bool` | sent when D gets busy with its own playback or another phone (§3.7) |
| both | `Bye` | `reason` (`quit`, `helper_off`, `sleeping`, `unpaired`, `engine_switch`) | the other side reacts at once instead of waiting for a timeout |

- **Versioning:** major in the prologue (a different major refuses with `HelperIncompatible`, showing which app to
  update); minor in `Hello` (new optional fields only).
- **Flow control:** the phone keeps at most **12 items or ~3 min of estimated 1× audio in flight** (whichever is less) and
  sends more as clips arrive. Jumps then waste at most one item of desktop work, and a cancel is cheap.
- Limits enforced by the decoder on both sides: frame ≤ 65 535 B, message ≤ 2 MB after reassembly, 32 items per batch,
  4 KB of text per item, clip ≤ 120 s of audio and ≤ `max(10 s, chars ÷ 6)` seconds (a sanity bound against a broken or
  hostile peer), sample rate equal to the `Ready` value.
- **More limits (security review pass 3, core `CHANGELOG.md`; built in `lectrice-link`):**
  - sample rates outside **8 000–48 000 Hz** are refused in `Welcome`, `Ready` and `Clip`; `decode_clip` never sizes its
    output past the longest clip the protocol allows;
  - per phone the computer keeps at most **256 waiting items** (`MAX_QUEUED_ITEMS_PER_PHONE`), makes no more clips while
    **8 MB** of them wait unread (`CLIP_BACKLOG_BYTES`, answered `HelperBusy` 5006), and ends a session with **16 MB**
    unwritten (`MAX_OUTBOX_BYTES`); the item budget (240 per minute) answers `HelperBusy` too;
  - at most **4 handshakes under way per address** (`MAX_HALF_OPEN_PER_ADDRESS`, 16 in all); the per-address back-off
    list holds ≤ 1 024 entries, forgets old failures and stops sessions only (a forgotten phone can pair again at once);
  - pairing: the phone is saved **before** it is told it is paired, and only while its window is still open (two right
    codes in one window save one phone); one attempt per address at a time; late wrong codes still count per hour;
  - the phone ends a session on unasked `Ready` / `Refused` answers and keeps at most 8 unread `Load` events; it pings
    after every frame too, so a long stream of clips never looks like a silent phone;
  - ids, versions, revisions, voice ids and error names with control or direction characters are refused;
  - reconnects (`LinkSpeechHelper`): a session counts as stable only after 30 s (2 × `dead_after`); shorter ones keep
    the 1-2-5-10-30-60 s schedule; the first `Bye(engine_switch)` in a row is answered at once.

### 3.6 The 30-minute read-ahead and the fallback (no gap)

**Horizons** (all in listening time, scaled by speed as everywhere, README §4.7):

| Link state | Who makes what | Horizon |
|---|---|---|
| **Connected** | `HelperFill` (C43) plans `HELPER_AHEAD_MINUTES = 30` × rate of 1× audio after the current sentence, in playback order, nearest first, and sends the uncached ones to the desktop. The local read-ahead plans nothing (`ReadAhead` paused for the helper's book, C44). The local engine stays **unloaded** (C46) | 30 min |
| **Lost** (§3.9), green phone (sustained `rtf × rate ≤ 0.6`, file 12 §3.3) | local `ReadAhead` at its normal setting (10 min). The 30 cached minutes mean it has nothing to do for ~20 minutes; it starts making only when the buffer ahead falls under 10 min. This saves the phone's battery in case the link comes back | 10 min (setting) |
| **Lost**, amber/red phone, or no stats yet | local `ReadAhead` at **30 min** at once, so it starts making **at the far end of the buffer** immediately and stretches it as long as possible | 30 min |
| **Off** (feature off / no computer paired) | today's behaviour | setting |

- The engine is **preloaded** as soon as the link goes Lost (≈ 1 s on the S10+-class, file 08), so it's ready long
  before the buffer runs low.
- **Why this never gaps:** the player only ever reads the cache or the local engine. The helper can only *add* clips. A
  link drop costs at most the clips in flight (≤ 3 min of desktop work); the phone has ~30 listening-minutes cached.
- How long the buffer lasts after a drop: `B ÷ (1 − 1 ÷ (rtf × rate))` when `rtf × rate > 1`, else forever. Example:
  S10+ with Fast (sustained RTF 1.11) at 1×, B = 30 min → the phone adds 0.9 min per minute → **~5 h** before
  "Catching up…" (vs ~100 min if it had waited until 10 min were left: hence the amber/red rule above).
- **When the buffer will run out** on a slow phone (`rtf × rate > 0.95` and < 10 min left), the existing slow-phone path
  applies (file 12 §3.3: tip, lower speed, Prepare) plus, **if file 23's audiobook source has a matching audiobook for this
  book**, one extra action "Play from the audiobook file" (file 23 decides the switch; never automatic).
- **Cache limits:** `keep_minutes_per_document = 60` must stay ≥ ahead × 1.5 (`MIN_KEEP_TO_READ_AHEAD_RATIO`): 30 × 1.5 =
  45 ≤ 60 ✓. The window keeps ~30 min ahead + ~30 min behind. At 2×: 60 min of 1× ahead ≈ **170 MB** WAV, window ≈ 340 MB,
  under the 1 GB cap ✓. No cap change.
- The **near prefetch** (ExoPlayer's own next items, file 08 §3.1) is unchanged: those `speak_sentence` calls hit the cache.

### 3.7 A sentence the player needs *now* (cache miss while Connected)

1. `SpeakSentence` misses the cache (after a jump, a voice change, the very first play).
2. It asks `HelperFill` to send that sentence **urgent** (front of the desktop's queue; if it's already queued there, it's
   moved up; an item already sent urgent is not sent again, since the computer would make it twice) and waits up to
   **`HELPER_NOW_GRACE` = 1.5 s** (H0: the laptop's p95 for a 10-word sentence is 1.09 s), counted **from the moment the
   link is ready**. While the phone's engine is still unloaded (C46) the wait is **`HELPER_COLD_GRACE` = 3 s**: the local
   path would pay the model load (2.2 s on the S10+) first, and a cold computer answered in 1.95–2.06 s (H4). (H5)
   - **First connection:** for **`HELPER_CONNECT_WAIT` = 1.5 s** after the helper starts, a miss waits for the link while
     it is still Connecting, and the local read-ahead stays paused (H4 saw play pressed ~100 ms before the link was ready,
     which loaded the engine). A reconnect never waits. A computer that isn't there costs the first sentence ≤ 1.5 s.
   - When the link becomes ready while the player waits, the listener's own sentence is the **first item, urgent**, ahead
     of the 30-minute fill.
3. If the clip arrives in time → play it. Otherwise (or the link is Lost) → load the local engine if needed and submit a
   local `Now` job (today's path). Whichever finishes first is cached; the second result is dropped (same key; the cache
   write is atomic, first writer wins, C45).
4. After a jump, `HelperFill` re-plans from the new sentence and cancels the desktop's stale queue (`Cancel`), like
   `stop_read_ahead` does locally.

Desktop-side priorities (one synthesis at a time on the computer; desktop spec §3.3): the desktop's own `Now` > phone
`urgent` > desktop read-ahead > phone fill > exports. When the desktop's own user plays, the phone gets `Load {
own_listener: true }` and keeps playing from its buffer; nothing changes for the phone unless the buffer gets low, in which
case §3.6's local rules take over as if Lost (but the session stays open).

### 3.8 Audio on the wire

**Recommendation: FLAC (lossless), level 5, of the same 16-bit 24 kHz mono PCM the cache stores.** The phone decodes it
and writes the normal WAV clip, so the cache stays one format and every clip is first-generation (an export re-encodes
to AAC only once).

Measured on the 41 s `af_heart` sample (24 kHz, 16-bit mono) on this laptop, 2026-11-13:

| Format | Bitrate | 30 listening-min at 1× | Notes |
|---|---|---|---|
| PCM 16-bit (WAV) | 384 kbit/s | 86 MB | today's cache format |
| **FLAC level 5** | **≈ 207 kbit/s** (54 % of PCM) | **≈ 47 MB** | lossless; pure-Rust encoder `flacenc` (Apache-2.0) on the desktop, decoder `claxon` (Apache-2.0) on the phone |
| AAC-LC 64 kbit/s | 64 kbit/s | 14 MB | lossy; export would then be AAC → AAC (two generations) |
| Opus 32 kbit/s | ≈ 32 kbit/s | 7 MB | lossy; needs libopus (C) or MediaCodec; same double-generation issue |

- The rate that matters is **while filling**: the desktop makes audio at ≈ 4.4× real time on the CPU (measured
  2026-10-21, fp32, 8 threads), so FLAC flows at ≈ 0.9 Mbit/s while filling (PCM would be 1.7 Mbit/s), then only ≈ 0.2
  Mbit/s (0.4 at 2×) to keep up with listening. Even a weak home Wi-Fi link (a few Mbit/s at the edge of range) carries
  that; a 30-minute fill takes ~7 min at 1× and ~14 min at 2× (desktop-bound, not network-bound).
- Lossy codecs were rejected: 4–6× less traffic doesn't matter at these rates, and lossless keeps desktop clips
  indistinguishable from phone clips in the cache.

### 3.9 Link states, heartbeats and reconnection

| State | Meaning | Enters | Leaves |
|---|---|---|---|
| `Off` | feature off, or no paired computer | setting | setting on + a pairing |
| `Searching` | NSD browsing; trying the last known address in parallel | play pressed (or the governor becomes Full/Reduced) on an allowed network (§3.10) | connected, or after 10 s → `Lost` |
| `Connecting` | TCP + Noise IK (timeout 5 s) | address found | `Connected` / `Lost` / `NeedsAttention` |
| `Connected` | `Ready` received for the phone's engine | | |
| `Busy` | connected, but the desktop's own listener or another phone has priority (`Load`) | `Load` | queue drains |
| `Lost` | not reachable; retrying | TCP error, 3 missed pongs (15 s), `Bye(quit/sleeping/helper_off)`, Wi-Fi lost callback | reconnect |
| `NeedsAttention` | won't work until the user acts: `HelperNotPaired` (forgotten on the desktop), `HelperModelMissing`, `HelperIncompatible`, permission denied | refusal | user action, then retry |

- **Retry schedule** (only while the governor is Full or Reduced, i.e. playing or just paused): 1, 2, 5, 10, 30 s, then
  every 60 s. **At once** on: Wi-Fi connected / network changed callback, a fresh NSD sighting of the computer's `id`,
  play pressed. Not at all while the governor is Off (no battery spent on reconnecting while nobody listens).
- On reconnect: `Open` again (the desktop may have switched models), re-plan from the current sentence; everything
  already cached is skipped, so nothing is made twice.
- **Idle close:** with no items in flight for **5 minutes** (paused, or the 30 min are full and the next item isn't due
  yet), the phone sends nothing but pings; after the governor goes Off it closes the session (`Bye`), letting Wi-Fi sleep.
  Reconnecting costs ~50–200 ms (one TCP connect + one Noise round trip).
- **Address changes:** the desktop's IP can change (DHCP); the phone finds it again by its `id` in the mDNS TXT record.

### 3.10 Network, battery and Android rules

- **Wi-Fi/Ethernet only.** `ConnectivityManager` callback for a network with `TRANSPORT_WIFI` or `TRANSPORT_ETHERNET`; the
  Rust socket is bound to that network (`android_setsocknetwork(network.networkHandle, fd)`, via the handle Kotlin passes,
  C49), so an active VPN or mobile data can never carry it. A metered Wi-Fi is fine (LAN traffic costs nothing).
- **Local network permission.** Today the app targets SDK 36, so `INTERNET` implicitly covers LAN access. **When it moves
  to target SDK 37 (Android 17)**, LAN access needs the runtime permission **`ACCESS_LOCAL_NETWORK`** (group "Nearby
  devices"). The system's NSD *picker* (`DiscoveryRequest.FLAG_SHOW_PICKER`) avoids the permission, but only for the device
  picked in that dialog in that moment; automatic reconnects need the permission. So: declare it, and **ask only when the
  user turns the helper on or taps "Pair"**, with a one-line rationale first ("Lectrice needs to find your computer on
  this Wi-Fi."). Denied → the helper row shows "Allow Nearby devices to use your computer" with a button to the app's
  permission page; nothing else in the app changes. (Source: Android's "Local network permission" page, checked
  2026-11-13.) No `CHANGE_WIFI_MULTICAST_STATE` is needed: `NsdManager` does the multicast.
- **Doze and the screen off.** Transfers happen while `PlaybackService` is a foreground `mediaPlayback` service (playing).
  The platform lets foreground-service processes use the network in Doze (UNVERIFIED on One UI: H0 runs `adb shell dumpsys
  deviceidle force-idle` during a fill on the S10+). While paused ≥ 60 s the governor is Off and nothing is transferred, so
  Doze can't hurt. A `WifiLock` is held **only while items are in flight** (mode chosen in H0; low-latency locks only work
  with the screen on, UNVERIFIED which mode One UI honours with the screen off).
- **Governor (file 08 §3.3) in helper mode:**

| Governor mode | Helper | Local engine |
|---|---|---|
| Full | fill to 30 min | idle (fallback only) |
| **Reduced** (battery saver, ≤ 15 %, thermal SEVERE) | **still fills to 30 min**: the radio costs a tiny fraction of synthesis, and saving the phone's battery and heat is the point of this mode | Reduced rules as today (2 min) if it has to run |
| Off (paused 60 s, sleep timer, no book, low memory, thermal CRITICAL) | `Cancel(all)`, then idle close | today's rules |

- **Battery effect:** the phone's biggest cost (synthesis: 8.8–36.9 mAh per audio-minute on the test phones, HANDOFF) goes
  to ~0 while Connected; a 30-min fill moves ~47 MB, a few seconds of active Wi-Fi at home speeds. Expected saving
  **> 80 %** of the playback-time battery use on the S10+ (UNVERIFIED; measured in H5).

### 3.11 The engine id rule (caches never mix)

- A desktop clip is cached **only if** the desktop's `Ready` reports **the same `engine_id` and the same `revision`** as the
  phone's own engine. The phone's key uses its own engine id, so a desktop clip is indistinguishable from a local one by
  design, and an unequal engine can't produce one.
- `engine_id` = the bundle id the apps already share (`kokoro-v1.0` Full quality, `kokoro-v1.0-lectrice-fast-v1` Fast,
  `kokoro-v1.0-lectrice-small-v1` Small; desktop model contract §5). **`revision`** (new, C42) = what changes the sound for
  the same model: the adapter's front end (sherpa-onnx version + the espeak-ng data / lexicon set), e.g.
  `"sherpa-onnx-1.13.8/kokoro-1"`. Both apps pin the same core tags, so they normally match; a mismatch means one app is
  older.
- Same model, different machine (x86 desktop vs ARM phone): the numbers aren't bit-identical, but it is the same model and
  front end, so it's the same voice. H0's ear test checks one sentence made on both.
- **The desktop has a different model** (e.g. only Full quality while the phone uses Fast): the desktop answers
  `Refused(HelperModelMissing, bundle_id)`. The phone shows **"Your computer needs the Fast voice (111 MB) to help. Open
  Lectrice on the computer to download it."**; the desktop shows a card **"Your phone uses the Fast voice. Download it
  here (111 MB) so the computer can help?"** [Download] (desktop spec §3.4). The desktop never downloads without its user's
  click (setting "Download voices my phone needs" can make it automatic, off by default). The phone **never** switches its
  own model to match the computer (its fallback must be the same voice).
- **Revision mismatch** → `Refused(HelperIncompatible)`: "Update Lectrice on your computer" (or "…on this phone"), decided
  by comparing `app_version`s.
- **Voices:** the phone's per-book voice is sent with every item; all our bundles have the same voice ids. A voice the
  desktop doesn't know → `ItemFailed(VoiceUnknown)` → the phone makes it locally and logs it.

### 3.12 How it fits the "audio source" abstraction (file 23)

File 23 (play from an exported audiobook) introduces a per-book **audio source** for the player. This feature does **not**
add a source; it adds a **producer** inside the existing one:

| Audio source (file 23) | Producers | Fallback order when a sentence is missing |
|---|---|---|
| **Sentence clips** (today's player: `SentenceDataSource` → `speak_sentence` → cache) | local engine (always), **desktop helper (this file)**, Prepare jobs (file 14; can run through the helper, §3.13) | cache → helper (urgent, ≤ 1.5 s, only when Connected) → local engine |
| **Audiobook file** (file 23) | none (the file is the audio) | n/a; file 23 owns switching between the two sources |

Rules so the two features fit:

- The helper never touches the audiobook source and the audiobook source never asks the helper. Switching a book to the
  audiobook source makes `HelperFill` stop for that book (`Cancel(all)`), like opening another book.
- File 23's "which source is active" state and this file's link state are separate values the pill reads; the pill shows
  at most **one** status mark (§11.2: the audiobook mark wins, since then the helper does nothing).
- When file 23 lands, its author checks these three points; any rename there (e.g. its word for "source") is applied here
  without changing the behaviour.

### 3.13 Prepare and export with the helper

- **Prepare (file 14) — in v1.** While Connected, a Prepare range (core C21) is sent through `HelperFill` instead of the
  local worker, and it no longer needs the charger (the phone isn't working). New player-sheet action when Connected:
  **"Prepare on Kia's laptop: This chapter · Next 3 chapters · As much as fits"** (the cache cap limits it; ~5.8 h of 1× WAV
  in 1 GB). This is the "before I leave the house" button. If file 14 itself is not built (green phones), only this
  helper-backed variant is built, behind the helper's switch.
- **Export (file 10) — later (H-later).** An export already uses cached clips (file 10 §3.4 "cached share"), so a book
  prepared through the helper exports fast without new code. Sending export parts through the helper is a later step.

## 4. States

| State | Phone sees | Desktop sees |
|---|---|---|
| Not paired | Settings row "Computer helper · Off" | Phone tab, helper switch, "Pair a phone" |
| Pairing | code entry sheet; "Checking…" | code card with 2-min countdown |
| Wrong code | "That code didn't work. Check the code on your computer." (3rd, or no code showing: "No code is showing on Kia's laptop. On the computer, click Pair a phone to get a new code.") | "Someone typed a wrong code 3 times. Click Pair a phone to try again." (S4) |
| Searching / Connecting | nothing in the pill; sheet line "Looking for Kia's laptop…" | — |
| Connected, filling | pill `computer` mark; sheet "Making audio on Kia's laptop · 12 min ready" | Phone tab: "Galaxy S10+ · making audio" + a small status chip in the app header (desktop spec §6) |
| Connected, full | sheet "… · 30 min ready" | "Galaxy S10+ · connected" |
| Busy (desktop's own listening) | sheet "Kia's laptop is busy · 22 min ready" | — |
| Lost | sheet "Computer not reachable · 24 min ready"; mark gone; one snackbar per session | the phone disappears from "connected" |
| Needs attention | sheet line + Settings row with the specific text (§3.11, §3.2); the pill shows nothing | model card / update notice |

## 5. Edge cases

- **Jump far away while Connected:** cancel, re-plan; the first sentence goes urgent; ≤ 1 desktop item wasted.
- **Speed change:** the horizon re-scales at the next plan (30 min at 2× = 60 min of 1× audio).
- **Voice change on the phone:** `Cancel(all)`, re-plan with the new voice (old-voice clips age out as today).
- **Model switch on the phone:** `Open` again with the new engine id (may be refused → NeedsAttention).
- **Model switch on the desktop mid-session:** desktop sends `Bye(engine_switch)`; the phone reconnects and `Open`s; if the
  phone's model is still installed there, it continues (the desktop helper engine is independent of its UI's active
  model, desktop spec §3.2).
- **Two phones, one computer:** both served, round-robin per item; urgent items first.
- **One phone, two paired computers:** the phone uses the first that answers `Ready` (the last used first); no splitting
  across computers in v1.
- **Laptop lid closed / sleep:** the desktop sends `Bye(sleeping)` when it can (it gets a suspend notice), otherwise the
  pongs stop; the phone goes Lost within 15 s. Desktop's own sleep is held off only while phone items are queued (desktop
  spec §5.3).
- **Wi-Fi with client isolation (guest networks, some mesh systems):** NSD finds nothing and connects fail; the pairing
  sheet's help text says "Phones and computers must be on the same Wi-Fi, not a guest network."
- **Phone on the computer's own hotspot (or vice versa):** works if both addresses are private (they are); fine.
- **Book removed (file 21) while filling:** `Cancel(all)`; late clips for it are dropped (the key's document no longer
  exists; `put` refuses, logged).
- **Cache cleared by Android while Connected:** the next plan sends everything again; nothing breaks.
- **Process death mid-transfer:** in-flight items are lost; on restart the plan re-sends them. Partially received clips
  are never written (the WAV is written only after the last chunk decodes and passes the bounds of §3.5).
- **Clock skew:** none of the protocol uses wall clocks (pings carry the sender's own monotonic time).

## 6. Where it lives

| Piece | Where |
|---|---|
| Protocol, framing, Noise sessions, SPAKE2 pairing, FLAC codec, client + server state machines | **core** new crate `lectrice-link` (C40, C41), features `client` / `server` |
| `SpeechHelper` port (no network words) | core `lectrice-domain` (C42) |
| `HelperFill` (30-min plan, flow control, urgent path), `ReadAhead` pause per book, Now-grace race, dedup | core `lectrice-app` (C43–C45) |
| Deferred engine (id/voices without loading the model) | core `lectrice-infra` (C46) |
| Registry names 5001–5005 | core `lectrice-domain/src/error.rs` (C47) |
| Server use case (queue, priorities, CPU gate, limits) | core `lectrice-app` + `lectrice-link` `server` (C48), used by the desktop |
| FFI: helper calls, status listener, key store foreign trait, network handle | `lectrice-android/rust/lectrice-ffi` (C49) |
| NSD discovery, permission, Wi-Fi network callback + binding, `WifiLock`, governor wiring, status → pill/sheet, Settings screens, Keystore-wrapped identity | Kotlin: new `core:link` module + `core:playback` (governor) + `feature:settings` (C50–C52) |
| Desktop server, Phone tab, firewall, sleep inhibit | `lectrice-desktop` (desktop spec) |

Clean architecture: nothing outside `lectrice-link`, the infra adapter and the composition roots knows about sockets,
Noise, mDNS or FLAC. `HelperFill` sees only the `SpeechHelper` port: "make these items, give me PCM back, tell me your
state". The player, read-ahead and export code don't know the helper exists.

## 7. Data and persistence

| What | Where | Notes |
|---|---|---|
| Own identity key pair | `filesDir/lectrice/link/identity.key` (AES-GCM with a Keystore key) | excluded from Auto Backup and device transfer (file 17 rules) |
| Paired computers | `filesDir/lectrice/link/peers.json`: `[{ peer_id, name, static_pub, last_address, paired_at, last_seen_at }]` (max 4) | public data only; written atomically |
| Settings | DataStore: `helperEnabled` (default **off** until paired; on after a successful pairing), `helperPreferredPeer` | |
| Clips | the normal audio cache (file 08 §7), same keys | nothing new on disk |
| Desktop side | desktop spec §4 | |

Logging (both sides): `link_state from=Connecting to=Connected peer=<peer_id 8 hex> rtt_ms=…`, `helper_fill planned=… sent=…
received=… bytes=… make_ms_p50=…`, errors by registry name. Never sentence text, titles or device names, at any log
level (device names are user data; peer ids are enough).

## 8. Decisions and open questions (recommendations are followed unless the user says otherwise)

| # | Question | Recommended answer (followed) |
|---|---|---|
| 1 | Pairing: code or QR? | **6-digit code on the desktop, typed on the phone, with SPAKE2.** QR later as an extra path. |
| 2 | Session crypto: Noise or TLS? | **Noise IK via `snow`** (one RTT, both keys pinned, no certificates). rustls with pinned self-signed certs was considered: more audited, but needs certificate generation and a custom verifier, and still needs SPAKE2 for pairing. Mitigation for `snow`/`spake2` not having a formal audit: official test vectors + fuzzing (§13), minimal protocol surface. |
| 3 | What does the phone send? | **Sentence speech text + sentence ids + voice**, never the document. |
| 4 | Wire audio format? | **FLAC** (lossless, ~207 kbit/s). |
| 5 | Desktop lacks the phone's model? | **Desktop offers to download it (its user clicks); the phone never switches models.** |
| 6 | Revision mismatch? | **Refuse with "update" text** (no mixing of front ends inside one book). |
| 7 | Read-ahead in helper mode? | **30 listening-minutes** (user), local engine idle; on loss: green phones wait (10 min horizon), amber/red start at once (30 min). |
| 8 | Battery saver / low battery in helper mode? | **Helper keeps filling to 30 min**; only local synthesis is reduced. |
| 9 | Helper without a model on the phone ("listen only through the computer")? | **Not in v1** (no fallback, no engine id). Later option. |
| 10 | Prepare through the helper? | **Yes, in v1** (§3.13); exports through the helper later. |
| 11 | Desktop runs the helper when its window is closed? | **v1: only while Lectrice is open** (minimized is fine); quitting tells the phone (`Bye`). Tray/background later (desktop spec §8). |
| 12 | Default port? | **47878/TCP**, any free port if taken (advertised by mDNS; shown for manual entry). |
| 13 | Feature on by default? | **Off until a pairing**; then on. |
| 14 | How many devices? | Phone: up to 4 computers; desktop: up to 8 phones. |

Nothing truly needs the user before building. One item to **tell** the user (not ask): when the Android app moves to
target SDK 37, turning the helper on will show Android's "Nearby devices" permission dialog.

## 9. Done when

- [x] Pairing works by mDNS and by typed address; a wrong code 3× ~~gives a new code~~ closes the window (S4); an unpaired
      phone is refused. (H4; H5: found in < 5 s and paired again. Typed address: tested in the core/desktop end-to-end
      tests, not on the phone.)
- [ ] With the helper Connected, 30 min at 1× and at 2× on the **S10+** play with no "Catching up…", the phone's engine
      stays unloaded (log), and the buffered part reaches 30 listening-minutes. (H5, **1× only**: 32 min with the screen
      off and Doze forced, every sentence from the cache, 0 local syntheses; H4: 28 min ready after 7.5 min. The model
      loaded by an earlier Lost now goes 5 min after the computer is back. **2× not run yet.**)
- [ ] Turning the router off mid-book: no audible gap; Lost within 15 s; S10+ (red) starts local work at once; after the
      router is back, Connected within 10 s and local work stops. (**The user tests this**: steps in
      `lectrice-android/docs/progress/H5.md`. Stand-ins on the S10+: computer frozen → Lost in 10.6 s, killed → Lost at
      once, local takeover, no gap (H4); back after 60–83 s → Connected 1.4 s / 6.7 s later (H5, 10 s re-browse).)
- [ ] Desktop quit / lid closed / helper switched off: same, with the `Bye` reason in the phone log. (H4 on the S10+:
      quit with its `Bye`, kill and freeze; helper off is in the desktop's tests. **Open:** the desktop never sends `Bye(sleeping)` (no suspend hook),
      so a closed lid is noticed as silence: Lost after ≤ 15 s, like H4's frozen computer; no gap either way.)
- [x] A jump while Connected plays the first sentence in ≤ 1.5 s (desktop) or falls back locally; no clip plays twice
      or out of order. (H5: play at app start while connecting 0.88 s, voice change 1.22 s, both from the computer, the
      phone's engine never loaded; the simulator checks order and no double clips.)
- [x] A desktop with a different model or older revision is refused with the right text on both sides. (H4; H5: "needs
      the Fast voice (140 MB)" on the phone, Fix → Retry → Connected.)
- [x] `tcpdump` on the laptop shows no plaintext after the prologue; logs on both sides contain no sentence text. (H5:
      no root for `tcpdump`, so a recording relay between phone and computer: 161 MB in 13 sessions, only the
      `lectrice-link/1 session|pair` prologues are readable, 0 book text (positive control works); both logs: 0 book
      phrases. Sockets: the app's only socket is the link; the laptop talks to the phone only on the link port and adb.)
- [x] Battery: 30 min at 1× on the S10+ with the helper uses ≤ 20 % of the battery the same 30 min use without it. (H5,
      screen off, batterystats estimate for the app with the charger simulated off: helper 1.53 mAh/min over 32.6 min,
      phone alone 16.2 mAh/min over 10.4 min → **9 %**. H4, screen on, measured: 21 %.)
- [x] Doze forced during a fill (screen off, playing): transfers continue. (H5: 32 min in deep IDLE, no Lost, no miss.)
- [ ] Every link-drop test of §13 passes in CI.

## 10. Design screens

New, need a design pass (§11 is the input; Opus brief → Fable options → user picks): phone **H1** Settings → Computer
helper (screen + rows), **H2** pair: computer list, **H3** pair: code entry (+ wrong-code, expired), **H4** pair: enter
address, **H5** paired success, **H6** permission rationale, **H7** pill mark + player-sheet helper line (all link states),
**H8** "Prepare on Kia's laptop" sheet row, **H9** Needs-attention variants (model missing, update, not paired).
Desktop frames: desktop spec §6.

## 11. UX notes for the design brief (phone)

Not mockups; what each screen must do and say. Copy marked *new, Opus*; it goes through `strings.xml`.

### 11.1 Settings → "Computer helper"

- A row in Settings under the voice model rows: title **"Computer helper"**, meta "Off" / "Kia's laptop · connected" /
  "Kia's laptop · not reachable". Tap → the H1 screen.
- H1 top: one switch **"Let a computer make the audio"** (disabled with "Pair a computer first" until a pairing exists),
  then one sentence: "When your phone and computer are on the same Wi-Fi, Lectrice on your computer makes the speech, so
  your phone stays cool and saves battery. If the computer isn't reachable, your phone carries on by itself."
- Section **"Computers"**: one row per paired computer (name, "Connected" / "Last seen 2 h ago"), long-press or ⋯ →
  "Rename" (local label only) / "Forget". Under it **"Pair with a computer"** (primary text button).
- Small help line at the bottom: "Needs Lectrice open on the computer, with 'Let my phone use this computer' turned on."
- No visible scrollbars; standard Quiet rows (README §4.3).

### 11.2 Status in the pill and the player sheet

- **Pill:** the status area (file 05) gets an optional 16 dp `computer` glyph before "Ch. 7 · p. 265" **only while
  Connected or Busy**. Nothing for Lost (the player sheet says it); nothing new in the collapsed pill. TalkBack: the
  pill's description adds ", audio from Kia's laptop".
- **Player sheet:** one line under the speed/voice rows, `computer` icon + text: "Making audio on Kia's laptop · 12 min
  ready" / "Kia's laptop is busy · 22 min ready" / "Computer not reachable · 24 min ready" / "Looking for Kia's laptop…".
  Tap → H1. When the line is a Needs-attention state it uses `warnFg` and its action ("Fix").
- **"min ready"** = listening-minutes buffered ahead at the current speed (`ready_ahead_ms ÷ rate`), rounded down; "under
  a minute ready" below 60 s.
- One snackbar per session on Connected → Lost while playing (§2). Never a dialog; never interrupts playback.
- The slider's buffered stretch (file 05) already shows the 30 minutes; no new graphic.

### 11.3 Pairing (H2–H6)

- H2 "Pair with a computer": a live list of found computers (name only; the TXT record has no pairing flag since S5); an empty state with a
  spinner "Looking on this Wi-Fi…" and after 10 s the help text (same Wi-Fi, not a guest network, Lectrice open, "Pair a
  phone" clicked) and the link **"Enter the address instead"** (H4). Not on Wi-Fi → "Connect to Wi-Fi to pair."
- H3 code entry: 6 separate digit boxes, numeric keyboard, paste-friendly, auto-submit on the 6th digit; subtitle "Type the
  code shown on Kia's laptop." States: Checking… (spinner), wrong code (error text under the boxes, boxes cleared),
  expired ("This code has expired. Get a new one on your computer."), 3rd failure.
- H4 address: one field "192.168.1.20:47878" with IP keyboard, then the same H3.
- H5 success: check mark, "Paired with Kia's laptop", the switch already on, button "Done".
- H6 permission rationale (SDK 37+ only): one sheet before the system dialog: "To find your computer, Lectrice needs
  permission to see devices on your Wi-Fi. It never looks beyond your home network." [Continue] [Not now].

### 11.4 Prepare through the helper (H8)

- In the player sheet when Connected: row `bolt` **"Prepare on Kia's laptop"** → chips "This chapter · Next 3 chapters ·
  As much as fits"; progress "Preparing · 40 %"/"Ready up to Ch. 9" as file 14.

## 12. Milestones

| Milestone | Content | Done when |
|---|---|---|
| **H0 spike** (2–3 days) | desktop RTF for Fast/Small/Full on the laptop CPU; first-sentence latency → `HELPER_NOW_GRACE_MS`; FLAC ratio over 30 min of speech; RAM with a second engine on the desktop; NSD on the S10+; Doze `force-idle` with a foreground `mediaPlayback` service; `WifiLock` mode; Windows firewall prompt on a per-user install; ear test of one sentence made on the laptop vs the S10+ | a short report `docs/progress/H0.md` (main repo) |
| **H1** core `lectrice-link` (C40, C41, C47) | protocol, framing, Noise IK + XXpsk3, SPAKE2 pairing, FLAC, server/client state machines, limits, fuzz targets | in-process client↔server tests + vectors + fuzz smoke green |
| **H2** core app (C42–C46, C48) | `SpeechHelper` port, `HelperFill`, urgent path + grace race, `ReadAhead` pause, deferred engine, `ServeSpeechHelper`, priorities | app tests incl. the link-drop simulator (§13) green; core tag **v0.4.0** |
| **H3** desktop | server in the composition root, Phone tab, pairing card, status chip, firewall/network checks, sleep inhibit, model-download card | desktop spec §9 |
| **H4** Android (C49–C52) | ffi, `core:link` (NSD, permission, network binding, Keystore), governor wiring, Settings H1–H6, pill/sheet H7, Prepare H8 | builds; unit tests |
| **H5** device tests (S10+ only; **the S26 stays out of tests**) | §9 list on the S10+ + the laptop (Linux) and the user's Windows PC | §9 all ticked |
| **H-later** | QR pairing, desktop tray/background, helper-only listening, exports through the helper, desktop GPU engine | each its own decision |

**Effort (agent work, Opus):** H0 2–3 d · H1 6–8 d · H2 4–5 d · H3 4–5 d · H4 6–8 d · H5 2–3 d + the design pass
(brief + Fable options + build) ~3 d → **about 5–6 weeks** in total.

### 12.1 As built: deviations from this file (H2–H5)

- **`item_id`** is `HelperFill`'s own counter, never the sentence id (§3.4/§3.5): a late clip after a voice change or a
  jump can never land under another key. **`Busy`** keeps filling (the phone's buffer covers it). (H2)
- **Pairing:** no `pair=` TXT record (S5); the 3rd wrong code closes the window and the computer's user clicks "Pair a
  phone" again, ≤ 10 failures per hour (S4). The phone's text after wrong codes is the generic "No code is showing on …"
  (§4). (H1/H4)
- **First sentence of a session** (§3.7): connect wait, grace from link-ready, cold grace, listener's sentence first. (H5)
- **The phone's model after a Lost** (§3.6 preload): playback holds the engine, so its 5-minute idle unload never comes
  while listening; the hub drops the deferred model once the computer has been back for 5 minutes. (H5)
- **Desktop:** the phone's engine is loaded when its session opens (before `Ready`; the desktop's own engine is lent and
  always loaded), and a helper engine is not idled out while a phone is connected (H5).
- **Re-discovery (Android):** NSD doesn't report again a computer that vanished without a goodbye, so while Lost the
  phone browses again every **10 s**; a fresh sighting of the computer's `id` triggers `retry_now`. (H4)
- **Prepare on the computer** (§3.13): the computer holds one Prepare range at a time, so the phone sends one book's
  range at a time (a book that reached Ready stays Ready); a range the computer stopped short of its end (`HelperBusy`) is
  sent again after 30 s, not on the next tick; the link is re-decided when "Prepare runs on the computer" flips. (H4)
- **Needs attention, model missing:** the phone names its own model and its download size ("needs the Fast voice
  (140 MB)"): it opened the link with that model, so it is the one the computer lacks (`HelperState.neededModel`, H5).
  "Update the older one" stays generic (the core doesn't say which side is older). (H4/H5)
- **"Last seen"** is stamped when a session ends and when the computer stops making audio, and shown relative at draw
  time. The ⋯ menu is a Compose `Popup`; the code boxes clear on every error; H5 (paired success) shows the live
  switch. (H4)

## 13. Tests

**Core (`lectrice-link`, `lectrice-app`), all in CI, no network beyond loopback:**

- Codec: round-trip every message; reject over-size frames, too many items, too-long text, bad chunk order, clips over the
  bounds of §3.5. **Fuzz** (`cargo fuzz`) the frame decoder, the message decoder and the FLAC path.
- Crypto: Noise vectors (cacophony / snow's own) for IK and XXpsk3; SPAKE2 vectors; wrong code fails at the PSK step;
  replayed or re-ordered frames end the session; an unpaired static key is refused; a peer address outside the private
  ranges is refused.
- **Link-drop simulator:** a test proxy between client and server (`FaultyLink`) that can **cut** the link (both ways or one
  way), **stall** it (packets held, no RST), **throttle** it (e.g. 128 kbit/s), add **latency/jitter** (300 ± 200 ms), and
  **cut mid-chunk**. A simulated player consumes clips at `rate` × real time on a **virtual clock** (hours in seconds).
  Assertions: (1) zero underruns while the buffer is non-empty, for cuts at random times over 1 000 runs; (2) Lost within
  15 s of a stall; (3) local work starts at once for a "red" fake engine and only below 10 min for a "green" one; (4) no
  sentence is cached twice with different bytes, none out of order, no partial clip written; (5) after the link comes
  back, the plan resumes and the local worker stops; (6) desktop work wasted per jump ≤ 1 item.
- Engine-id rule: a server reporting another engine id or revision → no clip is ever cached; the phone uses local.
- Priorities on the server: the desktop's own `Now` pre-empts queued phone items; urgent phone items pass fill items; two
  phones get round-robin service.
- Deferred engine: `SpeakSentence` with the helper Connected never loads the local model (counter in the fake factory).

**Android (JVM + instrumented):** governor decision table in helper mode (Reduced keeps the helper at 30 min, Off cancels);
link-state reducer (retry schedule, idle close, NeedsAttention); NSD → peer matching by `id`; Keystore round-trip;
permission flows on API 37 emulator images (denied, granted, revoked later).

**Device (H5, S10+ + laptop; also the Windows PC):** router off/on, walk out of range, airplane mode on/off, Wi-Fi switched
to another network, desktop quit / lid closed / helper off / model switch, Doze forced, 30-min battery comparison, `tcpdump`
plaintext check, the ear test. **Not on the S26.**

## 14. Security: threats and mitigations

| Threat | Mitigation |
|---|---|
| Someone on the Wi-Fi impersonates the computer (mDNS spoofing, ARP tricks) | discovery is only a hint; the session needs the pinned desktop key (Noise IK); a fake gets nothing and sends nothing we accept |
| Man-in-the-middle during pairing | SPAKE2: without the code the attacker can't complete the PSK handshake; one online guess per attempt, 3 attempts per code, 2-minute code |
| Passive listener on the Wi-Fi | everything after the prologue is ChaCha20-Poly1305; they learn only "Lectrice talks to a computer" and traffic volume |
| Brute-force or flood the desktop port | pairing only while the user shows a code; unpaired keys refused after one handshake; 5 s handshake timeout; ≤ 16 pending connections; per-address back-off after failures; limits on every message |
| A paired phone (or its stolen keys) abuses the desktop | worst case: makes the desktop synthesize text (CPU only); the desktop can "Forget" it; no file, library or shell access exists in the protocol |
| A hostile or buggy desktop sends bad audio | memory-safe Rust decoders, hard bounds on size/length/sample rate (§3.5), clips only ever become speech in the cache; worst case: wrong audio for a few sentences, cleared with "Clear audio cache" |
| Book text exposure | sent only encrypted, only to a paired device on the LAN, only the sentences being made; never stored or logged on the desktop |
| Traffic leaving the home network | private/link-local ranges only (CGNAT/VPN overlays refused), socket bound to the Wi-Fi network, no relay |
| Key theft from the phone | identity key encrypted with a non-exportable Keystore key; excluded from backups; "Forget" on the desktop revokes it |
| Downgrade to an older/weaker protocol | the version is in the prologue, which both Noise handshakes hash; only one cipher suite exists |
| Unaudited crypto crates (`snow`, `spake2`) | official vectors, fuzzing, no custom primitives (X25519, ChaCha20-Poly1305, BLAKE2s from RustCrypto/dalek), pinned exact versions |

## 15. Licensing

No impact. New crates are permissive and compatible with the GPL-3.0 build (espeak-ng): `snow` (Apache-2.0 OR MIT),
`spake2` (MIT OR Apache-2.0), `postcard` (MIT OR Apache-2.0), `flacenc` (Apache-2.0), `claxon` (Apache-2.0), RustCrypto /
dalek primitives (MIT/Apache-2.0/BSD-3). Desktop only: `mdns-sd` (Apache-2.0 OR MIT), `keepawake` (MIT). Android uses
the system `NsdManager`. All go into the generated third-party license lists. No Google, Qualcomm or cloud service.
FLAC itself is royalty-free.

## 16. Change list (C40–C52)

| Id | Change | Layer | Breaking? |
|---|---|---|---|
| C40 | New crate **`lectrice-link`**: wire protocol v1 (§3.5), framing, `postcard` messages, Noise IK sessions, limits, FLAC encode/decode, blocking client + server state machines over `std::net` with timeouts (no tokio needed on the phone) | core (new crate, features `client`, `server`) | no |
| C41 | Pairing: SPAKE2 + Noise XXpsk3, code lifetime/attempts, `PeerStore` + `IdentityStore` ports (shell-provided) | core `lectrice-link` | no |
| C42 | Domain port **`SpeechHelper`** (`state()`, `open(engine_id, revision)`, `make(items, sink)`, `cancel()`), items and PCM results; `SpeechEngine::revision()` with a default `""` (Kokoro adapter returns its front-end revision) | core domain + infra | no (default method) |
| C43 | **`HelperFill`** use case: 30-min plan (reuses `read_ahead_plan`), nearest first, flow control (12 items / 3 min), urgent path, jump/voice/book cancel, decode → existing `AudioEncoder` → `AudioCache::put` | core app | no |
| C44 | `ReadAhead` paused for a document while the helper is Connected; on Lost resumes with the horizon chosen by the shell (setting or 30 min) — `set_read_ahead_minutes` already exists (C14), add `pause_for_helper(doc)` / `resume(doc)` | core app | no |
| C45 | `SpeakSentence` urgent-through-helper with `HELPER_NOW_GRACE_MS`, then local; first writer wins (`AudioCache::put_if_absent`, default = `get` then `put`) | core app + domain | no (default method) |
| C46 | **Deferred engine** adapter: id, voices, sample rate from the bundle without loading sherpa-onnx; loads on the first `synthesize` and can preload | core infra | no |
| C47 | Registry names: `HelperNotPaired` 5001, `HelperPairingFailed` 5002, `HelperUnreachable` 5003, `HelperModelMissing` 5004, `HelperIncompatible` 5005 (Kotlin `ErrorName` + `ErrorRegistryTest` follow) | core domain | no |
| C48 | **`ServeSpeechHelper`** (server side): sessions per phone, priority `Helper` / `HelperUrgent` in `SpeechWorker`, one synthesis at a time on the machine across engines (CPU gate), round-robin between phones, `Load` messages, `EngineProvider` port so the desktop can lend its loaded engine or load a second one | core app + link `server` | no (new priority values) |
| C49 | FFI: `helper_configure(peers, identity_store)`, `helper_pair(addr, code, device_name) -> PeerRecord`, `helper_forget(peer_id)`, `helper_set_network(addr_candidates, network_handle)`, `helper_set_enabled(bool)`, `helper_set_ahead_policy(...)`, `helper_status() -> HelperStatusRecord { state, peer_id, peer_name, rtt_ms, ready_ahead_ms, busy, error }`, foreign trait `HelperListener.on_status`; socket bound with `android_setsocknetwork` | `lectrice-ffi` | no |
| C50 | Kotlin `core:link`: NSD browse/resolve, `ACCESS_LOCAL_NETWORK` (SDK 37+), Wi-Fi network callback, `WifiLock` while in flight, Keystore-wrapped `IdentityStore`, `PeerStore` file | Android | — |
| C51 | `ReadAheadGovernor` + `PlaybackService`: helper modes (§3.10 table), retry schedule, idle close, preload on Lost, Prepare via helper | Android `core:playback` | — |
| C52 | UI: Settings H1–H6, pill mark + sheet line H7, Prepare row H8, Needs-attention texts, strings | Android features | — |

Desktop-side changes (D-ids) are in the desktop spec §7.

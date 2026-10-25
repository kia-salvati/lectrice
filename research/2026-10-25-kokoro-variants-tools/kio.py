"""Tokenise invented sentences with the bundle's US lexicon (approximation of sherpa's Kokoro front end)."""
import re, numpy as np
M = "/home/kia/Desktop/learning/lectrice/models/kokoro-multi-lang-v1_0"
VOICES = ["af_alloy","af_aoede","af_bella","af_heart","af_jessica","af_kore","af_nicole","af_nova","af_river","af_sarah","af_sky","am_adam","am_echo","am_eric","am_fenrir","am_liam","am_michael","am_onyx","am_puck","am_santa","bf_alice","bf_emma","bf_isabella","bf_lily","bm_daniel","bm_fable","bm_george","bm_lewis"]
def _load():
    tok = {}
    for line in open(f"{M}/tokens.txt", encoding="utf-8"):
        line = line.rstrip("\n")
        if not line: continue
        sym, idx = line.rsplit(" ", 1)
        tok[sym if sym else " "] = int(idx)
    lex = {}
    for line in open(f"{M}/lexicon-us-en.txt", encoding="utf-8"):
        p = line.rstrip("\n").split(" ")
        if p[0] not in lex: lex[p[0]] = p[1:]
    voices = np.fromfile(f"{M}/voices.bin", dtype=np.float32).reshape(54, 510, 1, 256)
    return tok, lex, voices
TOK, LEX, VOX = _load()
def tokens(sentence):
    ids = []
    for w in re.findall(r"[A-Za-z']+|[,.;:!?\"]", sentence):
        if w in TOK and not w[0].isalpha():
            if ids and ids[-1] == TOK[" "]: ids.pop()
            ids += [TOK[w], TOK[" "]]; continue
        ph = LEX.get(w.lower())
        if ph is None: continue
        ids += [TOK[c] for c in ph if c in TOK] + [TOK[" "]]
    while ids and ids[-1] == TOK[" "]: ids.pop()
    return ids
def feeds(sentence, voice="af_heart", speed=1.0):
    ids = tokens(sentence)
    sid = VOICES.index(voice) if voice in VOICES else 3
    # sherpa's speaker order matches voices.bin order; English voices are the first 28
    style = VOX[sid, min(len(ids), 509)]
    return {"tokens": np.array([[0] + ids + [0]], dtype=np.int64), "style": style.astype(np.float32), "speed": np.array([speed], dtype=np.float32)}

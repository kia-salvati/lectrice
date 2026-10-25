"""Run a model in Python ORT on test.txt and write float WAVs (s00.wav...). opt=all|none."""
import sys, os, struct, time, numpy as np, onnxruntime as ort, kio
src, out, opt = sys.argv[1], sys.argv[2], sys.argv[3]
so = ort.SessionOptions(); so.intra_op_num_threads = 4
so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL if opt == "none" else ort.GraphOptimizationLevel.ORT_ENABLE_ALL
s = ort.InferenceSession(src, so, providers=["CPUExecutionProvider"])
os.makedirs(out, exist_ok=True)
tt = ta = 0
for i, l in enumerate(l for l in open("test.txt") if l.strip()):
    t = time.time(); a = s.run(None, kio.feeds(l))[0].ravel().astype("<f4"); tt += time.time() - t; ta += len(a) / 24000
    hdr = b"RIFF" + struct.pack("<I", 36 + a.nbytes) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 3, 1, 24000, 96000, 4, 32) + b"data" + struct.pack("<I", a.nbytes)
    open(f"{out}/s{i:02d}.wav", "wb").write(hdr + a.tobytes())
print(f"rtf {tt/ta:.3f}")

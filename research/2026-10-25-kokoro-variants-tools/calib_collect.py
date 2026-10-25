"""Lean min/max calibration: one sentence at a time, keep only per-tensor stats."""
import sys, json, numpy as np, onnx, onnxruntime as ort, kio
src, out = sys.argv[1], sys.argv[2]
OPS = {"Conv", "MatMul", "Gemm", "ConvTranspose"}
m = onnx.load(src)
inits = {i.name for i in m.graph.initializer}
vi = {v.name: v for v in list(m.graph.value_info) + list(m.graph.output) + list(m.graph.input)}
names = []
for n in m.graph.node:
    if n.op_type in OPS:
        for t in list(n.input) + list(n.output):
            if t and t not in inits and t not in names: names.append(t)
missing = [t for t in names if t not in vi]
print("tensors", len(names), "without value_info", len(missing))
existing = {o.name for o in m.graph.output}
for t in names:
    if t not in existing:
        m.graph.output.append(onnx.helper.make_tensor_value_info(t, onnx.TensorProto.FLOAT, None))
so = ort.SessionOptions(); so.intra_op_num_threads = 8
so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
sess = ort.InferenceSession(m.SerializeToString(), so, providers=["CPUExecutionProvider"])
del m
outs = [o.name for o in sess.get_outputs()]
idx = {t: outs.index(t) for t in names}
lines = [l.strip() for l in open("calib.txt") if l.strip()]
voices = ["af_heart", "af_bella", "af_nicole", "am_michael", "bf_emma", "af_sarah", "bm_george", "af_sky"]
stats = {t: [] for t in names}
for i, l in enumerate(lines):
    r = sess.run(None, kio.feeds(l, "af_heart" if i % 2 == 0 else voices[(i // 2) % len(voices)]))
    for t in names:
        a = r[idx[t]].ravel()
        q = np.quantile(a, [0.001, 0.999, 0.0001, 0.9999, 0.00001, 0.99999]) if a.size > 20000 else [a.min(), a.max()] * 3
        stats[t].append([float(a.min()), float(a.max())] + [float(x) for x in q])
    del r
    if i % 10 == 0: print("sentence", i, flush=True)
json.dump(stats, open(out, "w"))

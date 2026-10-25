import sys, json, collections, onnxruntime as ort, kio
so = ort.SessionOptions(); so.intra_op_num_threads = 4; so.enable_profiling = True
s = ort.InferenceSession(sys.argv[1], so, providers=["CPUExecutionProvider"])
f = kio.feeds("Most of the time, a simple design that everyone understands is better than a clever one that nobody can change.")
for _ in range(3): s.run(None, f)
p = s.end_profiling()
ev = json.load(open(p))
op = collections.Counter(); sec = collections.Counter()
for e in ev:
    if e.get("cat") == "Node" and e["name"].endswith("_kernel_time"):
        op[e["args"]["op_name"]] += e["dur"]
        n = e["name"]
        sec["/".join(n.split("/")[:4])] += e["dur"]
tot = sum(op.values())
print("total ms", tot/3000)
for k, v in op.most_common(12): print(f"{k:28s} {100*v/tot:5.1f}%")
print("--")
for k, v in sec.most_common(12): print(f"{k:40s} {100*v/tot:5.1f}%")
import os; os.remove(p)

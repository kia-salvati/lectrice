"""Weight-storage variants: big fp32 initializers stored as fp16 (Cast) or per-channel int8 (DequantizeLinear); compute stays fp32."""
import sys, re, numpy as np, onnx
from onnx import helper, numpy_helper, TensorProto
src, dst, mode = sys.argv[1], sys.argv[2], sys.argv[3]
MIN_ELEMS = 4096
m = onnx.load(src)
g = m.graph
consumers = {}
for n in g.node:
    for k, i in enumerate(n.input): consumers.setdefault(i, []).append((n, k))
def channel_axis(name, arr):
    uses = consumers.get(name, [])
    ops = {(n.op_type, k) for n, k in uses}
    if ("ConvTranspose", 1) in ops: return 1
    if ("Conv", 1) in ops: return 0
    if ("Gemm", 1) in ops:
        n = [n for n, k in uses if n.op_type == "Gemm"][0]
        tb = next((a.i for a in n.attribute if a.name == "transB"), 0)
        return 0 if tb else 1
    if ("MatMul", 1) in ops: return arr.ndim - 1
    if any(op == "LSTM" for op, _ in ops): return 1
    return 0
new_init, new_nodes, done = [], [], 0
for init in g.initializer:
    if init.data_type != TensorProto.FLOAT or np.prod(init.dims) < MIN_ELEMS:
        new_init.append(init); continue
    arr = numpy_helper.to_array(init)
    name = init.name
    use_int8 = mode == "int8" or (mode.startswith("mix:") and any(re.search(mode[4:], n.name) for n, _ in consumers.get(name, [])))
    if not use_int8:
        new_init.append(numpy_helper.from_array(arr.astype(np.float16), name + "_fp16"))
        new_nodes.append(helper.make_node("Cast", [name + "_fp16"], [name], to=TensorProto.FLOAT, name=name + "_cast"))
    else:
        ax = channel_axis(name, arr)
        red = tuple(i for i in range(arr.ndim) if i != ax)
        amax = np.abs(arr).max(axis=red)
        scale = np.where(amax > 0, amax / 127.0, 1.0).astype(np.float32)
        shp = [1] * arr.ndim; shp[ax] = -1
        q = np.clip(np.round(arr / scale.reshape(shp)), -127, 127).astype(np.int8)
        new_init += [numpy_helper.from_array(q, name + "_q"), numpy_helper.from_array(scale, name + "_scale"), numpy_helper.from_array(np.zeros_like(scale, dtype=np.int8), name + "_zp")]
        new_nodes.append(helper.make_node("DequantizeLinear", [name + "_q", name + "_scale", name + "_zp"], [name], axis=ax, name=name + "_dq"))
    done += 1
del g.initializer[:]; g.initializer.extend(new_init)
nodes = new_nodes + list(g.node)
del g.node[:]; g.node.extend(nodes)
if mode != "fp16" and all(o.version < 13 for o in m.opset_import if o.domain in ("", "ai.onnx")):
    raise SystemExit("needs opset >= 13 for per-axis DequantizeLinear")
onnx.save(m, dst)
print("converted", done, "initializers")

"""Static QDQ int8 from our own calibration stats (calib_stats.json)."""
import sys, re, json, os, numpy as np, onnx
from onnxruntime.quantization import quantize_static, QuantFormat, QuantType, CalibrationMethod
from onnxruntime.quantization.calibrate import TensorsData, TensorData, save_tensors_data
src, dst, cfg = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
stats = json.load(open("calib_stats.json"))
data = {}
for t, rows in stats.items():
    a = np.array(rows)
    r = cfg.get("range", "minmax")
    if isinstance(r, dict):
        r = next((v for k, v in r.items() if re.search(k, t)), r.get("default", "minmax"))
    col = {"p3": 2, "p4": 4, "pct": 4, "p5": 6}
    lo, hi = (a[:, 0].min(), a[:, 1].max()) if r == "minmax" else (a[:, col[r]].mean(), a[:, col[r] + 1].mean())
    data[t] = TensorData(lowest=np.array(min(lo, 0.0), dtype=np.float32), highest=np.array(max(hi, 0.0), dtype=np.float32))
cache = dst + ".calib.json"
save_tensors_data(TensorsData(CalibrationMethod.MinMax, data), cache)
m = onnx.load(src, load_external_data=False)
ops = cfg.get("ops", ["Conv", "MatMul", "Gemm"])
excl = [re.compile(p) for p in cfg.get("exclude", [])]
incl = [re.compile(p) for p in cfg.get("include", [".*"])]
nodes = [n.name for n in m.graph.node if n.op_type in ops and any(p.search(n.name) for p in incl) and not any(p.search(n.name) for p in excl)]
print("quantizing", len(nodes), "nodes")
quantize_static(src, dst, None, quant_format=QuantFormat.QDQ, op_types_to_quantize=ops, nodes_to_quantize=nodes,
    per_channel=True, activation_type=getattr(QuantType, cfg.get("act", "QUInt8")), weight_type=QuantType.QInt8,
    calibrate_method=CalibrationMethod.MinMax, calibration_cache_path=cache,
    extra_options={"ActivationSymmetric": cfg.get("act_sym", False), **cfg.get("extra", {})})
os.remove(cache)

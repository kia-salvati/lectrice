import sys, onnx
src, dst, variant, fp32sha = sys.argv[1:5]
m = onnx.load(src)
for k, v in {"lectrice_variant": variant, "lectrice_source_model_sha256": fp32sha, "lectrice_made": "2026-10-25 by Lectrice from sherpa-onnx kokoro-multi-lang-v1_0 model.onnx (Apache-2.0)"}.items():
    p = m.metadata_props.add(); p.key, p.value = k, v
onnx.save(m, dst)

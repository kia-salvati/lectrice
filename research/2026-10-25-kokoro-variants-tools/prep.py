import sys
from onnxruntime.quantization.shape_inference import quant_pre_process
quant_pre_process(sys.argv[1], sys.argv[2], skip_symbolic_shape=(sys.argv[3] == "skip"), skip_optimization=False)

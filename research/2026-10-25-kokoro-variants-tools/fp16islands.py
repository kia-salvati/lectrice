"""fp16 'islands': only the vocoder's heavy Conv nodes run in fp16 (Cast in, fp16 weights, Cast out); everything else stays fp32."""
import sys, re, collections, numpy as np, onnx
from onnx import helper, numpy_helper, TensorProto
src, dst, pattern = sys.argv[1], sys.argv[2], sys.argv[3]
m = onnx.load(src); g = m.graph
target = re.compile(pattern)
uses = collections.Counter(i for n in g.node for i in n.input)
init = {i.name: i for i in g.initializer}
new_nodes, count = [], 0
for n in g.node:
    if n.op_type == "Conv" and target.search(n.name) and all(i in init and uses[i] == 1 for i in n.input[1:]):
        x, y = n.input[0], n.output[0]
        for k in range(1, len(n.input)):
            t = init[n.input[k]]
            t.CopyFrom(numpy_helper.from_array(numpy_helper.to_array(t).astype(np.float16), t.name))
        new_nodes.append(helper.make_node("Cast", [x], [x + f"_h{count}"], to=TensorProto.FLOAT16, name=n.name + "_in16"))
        n.input[0] = x + f"_h{count}"; n.output[0] = y + "_h"
        new_nodes.append(n)
        new_nodes.append(helper.make_node("Cast", [y + "_h"], [y], to=TensorProto.FLOAT, name=n.name + "_out32"))
        count += 1
    else:
        new_nodes.append(n)
del g.node[:]; g.node.extend(new_nodes)
# stale shape info would carry fp32 types for the renamed tensors
del g.value_info[:]
onnx.save(m, dst); print("fp16 convs", count)

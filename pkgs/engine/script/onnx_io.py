import time
from pathlib import Path

import numpy as np
import onnxruntime as ort
import torch

from patches import SNAKE_DOMAIN, register_snake_pyop

OPSET = 18
TOL = 2e-3


def export(module, args, path: Path, input_names, output_names, dynamic_axes):
    t0 = time.time()
    path.parent.mkdir(parents=True, exist_ok=True)
    with torch.inference_mode(False), torch.no_grad():
        torch.onnx.export(
            module,
            args,
            str(path),
            input_names=input_names,
            output_names=output_names,
            dynamic_axes=dynamic_axes,
            opset_version=OPSET,
            dynamo=False,
            do_constant_folding=True,
            custom_opsets={SNAKE_DOMAIN: 1},
        )
    print(f"  exported {path.name} ({path.stat().st_size / 1e6:.0f} MB, {time.time() - t0:.1f}s)")


def verify(module, args_list, path: Path, input_names, tol=TOL):
    so = ort.SessionOptions()
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    so.register_custom_ops_library(register_snake_pyop())
    sess = ort.InferenceSession(str(path), so, providers=["CPUExecutionProvider"])
    for args in args_list:
        with torch.no_grad():
            ref = module(*args)
        refs = ref if isinstance(ref, tuple) else (ref,)
        feeds = {n: a.numpy() for n, a in zip(input_names, args)}
        outs = sess.run(None, feeds)
        for r, o in zip(refs, outs):
            err = float(np.max(np.abs(r.numpy() - o))) if r.numel() else 0.0
            scale = float(r.abs().max()) if r.numel() else 0.0
            status = "ok" if err <= tol * max(1.0, scale) else "MISMATCH"
            print(f"    verify {path.name} shape={tuple(o.shape)} max_err={err:.2e} (|ref|max={scale:.2f}) {status}")
            if status != "ok":
                raise SystemExit(f"verification failed for {path}")

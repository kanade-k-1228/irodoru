import os
import tempfile
import unittest
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

from portable_onnx import make_portable


class PortableModelTest(unittest.TestCase):
    def test_per_head_rmsnorm_scale(self):
        gamma = np.arange(20 * 64, dtype=np.float32).reshape(20, 64) / 512 + 0.25
        gamma[2, 57] = 24.75
        gamma[10, 25] = -2
        nodes = [
            helper.make_node("Mul", ["x", "x"], ["square"]),
            helper.make_node("ReduceMean", ["square"], ["mean"], axes=[-1], keepdims=1),
            helper.make_node("Add", ["mean", "eps"], ["denom"]),
            helper.make_node("Sqrt", ["denom"], ["root"]),
            helper.make_node("Div", ["one", "root"], ["inverse"]),
            helper.make_node("Mul", ["x", "inverse"], ["normalized"]),
            helper.make_node("Identity", ["norm.weight"], ["heads.norm.weight"]),
            helper.make_node("Mul", ["normalized", "heads.norm.weight"], ["y"]),
        ]
        graph = helper.make_graph(nodes, "per_head_rmsnorm",
            [helper.make_tensor_value_info("x", TensorProto.FLOAT, ["b", "s", 20, 64])],
            [helper.make_tensor_value_info("y", TensorProto.FLOAT, ["b", "s", 20, 64])],
            [numpy_helper.from_array(gamma, "norm.weight"),
             numpy_helper.from_array(np.array(1e-6, np.float32), "eps"),
             numpy_helper.from_array(np.array(1, np.float32), "one")])
        model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)], ir_version=10)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "norm.onnx"
            onnx.save(model, source)
            original = source.read_bytes()
            portable = make_portable(source)
            self.assertEqual(source.read_bytes(), original)
            options = ort.SessionOptions()
            options.intra_op_num_threads = 1
            cpu = ort.InferenceSession(str(portable), options, providers=["CPUExecutionProvider"])
            gpu = None
            if device := os.environ.get("IRODORI_TEST_DEVICE"):
                import openvino as ov
                gpu = ov.Core().compile_model(str(portable), device, {"INFERENCE_PRECISION_HINT": "f32"})
            for shape in [(1, 3, 20, 64), (3, 7, 20, 64), (1, 1, 20, 64)]:
                x = np.random.default_rng(7).normal(size=shape).astype(np.float32)
                x[0, 0] = 0
                expected = x / np.sqrt(np.mean(x * x, axis=-1, keepdims=True) + 1e-6) * gamma
                actual, = cpu.run(None, {"x": x})
                np.testing.assert_allclose(actual, expected, rtol=2e-6, atol=2e-6)
                if gpu is not None:
                    result = gpu({"x": x})[gpu.output(0)]
                    np.testing.assert_allclose(result, expected, rtol=2e-6, atol=2e-6)

    def test_snake_expansion_matches_formula_and_preserves_source(self):
        alpha = np.array([[0.0], [0.8]], dtype=np.float32)
        graph = helper.make_graph(
            [helper.make_node("Snake", ["x", "alpha"], ["y"], domain="ai.onnx.contrib")],
            "snake", [helper.make_tensor_value_info("x", TensorProto.FLOAT, ["b", 2, "t"])],
            [helper.make_tensor_value_info("y", TensorProto.FLOAT, ["b", 2, "t"])],
            [numpy_helper.from_array(alpha, "alpha")],
        )
        model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17), helper.make_opsetid("ai.onnx.contrib", 1)], ir_version=10)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "decoder.onnx"
            onnx.save(model, source)
            original = source.read_bytes()
            portable = make_portable(source)
            self.assertEqual(source.read_bytes(), original)
            self.assertFalse(any(n.op_type == "Snake" for n in onnx.load(portable).graph.node))
            options = ort.SessionOptions()
            options.intra_op_num_threads = 1
            session = ort.InferenceSession(str(portable), options, providers=["CPUExecutionProvider"])
            for shape in [(1, 2, 3), (2, 2, 19)]:
                x = np.random.default_rng(7).normal(size=shape).astype(np.float32)
                sine = np.sin(alpha * x)
                expected = x + (1 / (alpha + 1e-9)) * sine * sine
                actual, = session.run(None, {"x": x})
                np.testing.assert_allclose(actual, expected, rtol=1e-6, atol=1e-6)


if __name__ == "__main__":
    unittest.main()

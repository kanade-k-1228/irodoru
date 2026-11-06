"""Prepare equivalent ONNX graphs for accelerator runtimes.

Existing exports: python portable_onnx.py --model-dir ../../../models
The original CPU graphs are preserved; no checkpoint or PyTorch is needed.
"""

import argparse
from pathlib import Path

import numpy as np
import onnx
from onnx import helper, numpy_helper


def original_graphs(directory: Path, pattern="*.onnx"):
    return (path for path in sorted(directory.glob(pattern)) if not path.name.endswith(".portable.onnx"))


def unique_prefix(occupied: set[str], prefix: str) -> str:
    while any(name.startswith(prefix) for name in occupied):
        prefix += "_"
    return prefix


def normalize_head_scale(graph, node, gamma, weight, prefix):
    x = next(name for name in node.input if name != gamma)
    graph.initializer.extend([
        numpy_helper.from_array(weight.reshape(-1), prefix + "weight"),
        numpy_helper.from_array(np.array([-1, weight.size], dtype=np.int64), prefix + "flat_shape"),
    ])
    return [
        helper.make_node("Shape", [x], [prefix + "shape"]),
        helper.make_node("Reshape", [x, prefix + "flat_shape"], [prefix + "flat"]),
        helper.make_node("Mul", [prefix + "flat", prefix + "weight"], [prefix + "scaled"]),
        helper.make_node("Reshape", [prefix + "scaled", prefix + "shape"], list(node.output)),
    ]


def expand_snake(graph, node, prefix, offset):
    x, alpha = node.input
    eps = prefix + "eps"
    graph.initializer.append(numpy_helper.from_array(np.array(1e-9, dtype=np.float32), eps))
    operations = [
        ("Mul", [alpha, x], prefix + "ax"),
        ("Sin", [prefix + "ax"], prefix + "sin"),
        ("Add", [alpha, eps], prefix + "denominator"),
        ("Reciprocal", [prefix + "denominator"], prefix + "inverse"),
        ("Mul", [prefix + "inverse", prefix + "sin"], prefix + "scaled_sin"),
        ("Mul", [prefix + "scaled_sin", prefix + "sin"], prefix + "residual"),
        ("Add", [x, prefix + "residual"], node.output[0]),
    ]
    return [
        helper.make_node(op, inputs, [output], name=prefix + op + str(offset + index))
        for index, (op, inputs, output) in enumerate(operations)
    ]


def make_portable(source: Path) -> Path:
    model = onnx.load(source)
    graph = model.graph
    occupied = {name for node in graph.node for name in (*node.input, *node.output)}
    occupied.update(tensor.name for tensor in graph.initializer)
    weights = {
        tensor.name: numpy_helper.to_array(tensor)
        for tensor in graph.initializer if len(tensor.dims) == 2
    }
    nodes = []
    for index, node in enumerate(graph.node):
        if node.op_type == "Identity" and node.input[0] in weights:
            weights[node.output[0]] = weights[node.input[0]]
        gamma = next((
            name for name in node.input
            if name in weights and "norm" in name and name.endswith(".weight")
        ), None)
        if node.op_type == "Mul" and gamma is not None:
            prefix = unique_prefix(occupied, f"irodori_norm_{index}_")
            nodes.extend(normalize_head_scale(graph, node, gamma, weights[gamma], prefix))
        elif (node.domain, node.op_type) == ("ai.onnx.contrib", "Snake"):
            prefix = unique_prefix(occupied, f"irodori_snake_{index}_")
            nodes.extend(expand_snake(graph, node, prefix, len(nodes)))
        else:
            nodes.append(node)
    del graph.node[:]
    graph.node.extend(nodes)
    onnx.checker.check_model(model)
    destination = source.with_suffix(".portable.onnx")
    onnx.save(model, destination)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, required=True)
    args = parser.parse_args()
    for source in original_graphs(args.model_dir, "*/*.onnx"):
        print(make_portable(source), flush=True)


if __name__ == "__main__":
    main()

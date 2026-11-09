`add.onnx` is a 98-byte, project-authored ONNX fixture (MIT): IR version 8,
default-domain opset 13, one Add node with float32 inputs A and B and output Y,
all with shape [3]. It has no weights or external data. The Windows ML package probe runs
A=[1,2,3], B=[4,5,6] and requires Y=[5,7,9] with CPU fallback disabled.

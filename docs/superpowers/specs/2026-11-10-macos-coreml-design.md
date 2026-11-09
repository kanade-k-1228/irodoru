# macOS Core ML

The user approved applying the Windows-style accelerator preference and CPU
fallback to Mac on 2026-11-10. Extend the existing ORT backend with Core ML,
using the OS framework and the reviewed ORT archive. Keep the macOS 15 Apple
Silicon baseline. A separate model conversion/runtime would add deployment and
model-maintenance work; the selected ORT Core ML provider preserves current ONNX
models and custom CPU operations.

Enable Core ML only for macOS. Automatic sessions use MLProgram and all compute
units. Explicit CPU stays CPU-only; GPU/NPU restrict the allowed compute policy.
Failed registration, session creation or execution falls back in automatic mode.
Reuse the existing graph tracker and UI, showing Core ML selection without claiming
per-operation hardware placement. Document requirements and test both the shared
fallback machinery and Core ML registration/inference in Mac CI. No persistent
compiled-model cache is introduced. Hardware performance is a separate Mac
validation step.

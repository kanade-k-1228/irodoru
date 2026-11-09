# Windows ML distribution and inference

Implementation authorized by the user on 2026-11-10.

## Goal and selected approach

Ship one application executable plus license materials without app-local runtime
DLLs, while retaining GPU/NPU support. The Windows build loads the shared Windows
ML runtime from a registered Windows App SDK framework package. When the framework is unavailable, a statically linked CPU ONNX Runtime is selected
before any ORT objects are created. The application and CPU runtime CRTs are static. Models and execution providers are installed separately.

Alternatives considered: the original static ONNX Runtime with OS DirectML left
an older API baseline and native CRT dependency; embedding and extracting DLLs
would still deploy runtime DLLs. A CPU-only build was explicitly rejected.

## Implementation

- Windows 11 24H2 x64. GPU/NPU requires Windows App SDK Runtime 2.6.5 or newer within major 2; CPU requires no separate installation.
- Use OS dynamic-dependency APIs to attach the publisher-qualified framework
  family, then load DLLs using absolute paths from its registered directory.
  Retain package references, DLLs, and provider catalog for the process lifetime.
- Pin Rust ORT API compatibility to 27, based on the reviewed Windows ML 2.4.89
  C headers (the reviewed framework contains ONNX Runtime 1.27.1). Resolve the Windows ML catalog exports dynamically.
- Prepare certified execution providers through Windows ML and register each
  library with the application's own ORT environment. Windows defaults to Auto.
  Select GPU/NPU devices through ORT's device API. Keep CPU fallback for automatic
  mode, explicitly disabling auto-selection in the CPU session.
- Keep original model graphs and Snake custom operator registration. Unsupported
  operators may execute on CPU; successful session creation is not proof that
  the entire speech model is accelerated. Non-Windows behavior is unchanged.
- Reject runtime DLLs in packages; ignore only stale DirectML from old caches.
  Check direct PE imports for unwanted native runtime dependencies.
- Provide a checksum-pinned installer for shared runtime packages and use it in
  Windows CI. EP downloads happen on first accelerator discovery, not during the
  loader-only CI probe. Missing/incompatible framework selects embedded CPU.
- Build CPU ORT 1.31.0 from a pinned commit with full ONNX support and `/MT`.
  Preserve the existing native notices. No DirectML dependency is linked.
- Display selected graph providers in the footer and model settings. Keep live
  graph status handles so execution fallback updates the UI; discarded engines
  cannot leave stale provider entries. This is selection, not node profiling.

## Validation

Windows cross-target type checking/Clippy, native engine tests, package-policy
unit tests, ZIP-content tests, and formatting checks are executable locally.
Windows CI first probes embedded CPU, then installs the framework and starts only the packaged exe from a temporary
folder with system-only PATH, and runs a tiny Add graph on shared-runtime CPU.
GPU probe mode explicitly forbids CPU fallback for that Add graph.

Actual Windows loading, full speech generation, GPU/NPU performance, offline reuse,
and behavior across driver vendors require Windows runtime/hardware validation.
Do not report these as passed from cross-compilation or CPU-only CI results.

## Reviewed sources

- Microsoft.WindowsAppSDK.Runtime 2.6.5: package family, minimum version, signed
  framework/companion MSIX packages and their SHA-256 pinned in the installer.
- Microsoft.Windows.AI.MachineLearning 2.4.89: WinMLEpCatalog.h ABI and ORT API 27.
- https://learn.microsoft.com/windows/ai/new-windows-ml/register-execution-providers
- https://learn.microsoft.com/windows/ai/new-windows-ml/distributing-your-app

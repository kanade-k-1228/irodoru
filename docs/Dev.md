# Development guide

[README](../README.md) · [User guide](Usage.md) · [Docker](Docker.md)

## Setup

- Install stable Rust, Python 3.10 or later, uv, and GPUI Kit's native dependencies. Node.js is not required.
- For Linux packages, see the [CI configuration](../.github/workflows/app.yml). Running the app requires Vulkan, Wayland or X11, and `xdg-desktop-portal` with a desktop-specific backend.

Run from the repository root:

```bash
cargo xtask models export    # Export the anime model and codec
cargo xtask models prepare   # Prepare development models
cargo xtask dev              # Start the desktop app
```

If you already have models, replace the first two commands with `cargo xtask models prepare --source /path/to/models`. Alternatively, set `IRODORU_MODELS` to a directory containing `anime/` and `codec/`. The checked-in distribution manifest has placeholder values, so local development requires prepared models.

## Repository layout

- `pkgs/app`: Rust/GPUI desktop app. See [application architecture](../pkgs/app/README.md) for module boundaries and state flow.
- `pkgs/engine`: Shared speech engine using ONNX Runtime, with optional OpenVINO support. Consumers use the public types at the crate root; graph, codec, model, and cache internals stay private.
- `pkgs/model-bundle`: Distribution manifest types, required files, and path/size/hash validation shared by the app and xtask. It has no filesystem, network, GUI, or inference dependencies.
- `pkgs/engine/script`: Python model export and validation.
- `pkgs/server`: Axum HTTP API using the same engine as the app.
- `xtask`: Development, verification, packaging, and model distribution commands.
- `scripts` and `.github/workflows`: Packaging helpers and CI.

Use `cargo xtask models ...` locally; CI uses a release build of xtask. Model preparation, validation, ZIP creation, and GHCR publishing run in Rust. ONNX export calls Python with PyTorch.

## Verify

```bash
cargo xtask verify          # fmt, Clippy, OpenAPI diff
cargo xtask generate        # Regenerate OpenAPI
```

There are no automated tests. See CI for checks on packaged apps. Check Japanese input, dragging, and audio devices manually on each OS.

## Package and release

```bash
cargo install cargo-packager --version 0.11.8 --locked
cargo install cargo-about --version 0.9.3 --locked --features cli
cargo fetch --locked
cargo xtask package
```

- Linux also requires `patchelf`; Windows requires 7-Zip.
- Windows packages contain the executable and license materials, without runtime DLLs. They use Windows ML when available and an embedded CPU ONNX Runtime otherwise. Both Rust and the embedded runtime use a static CRT.
- Add `--no-bundle` to create an app directory without an installer. App packages do not include models.
- Use `cargo xtask models distribute` to create model ZIPs and distribution manifests.

Packaging uses `cargo-about` to inspect locked dependencies and include a shared `licenses/` bundle covering supported targets and build dependencies. Some listed components are absent on a particular OS. Generate it separately with `cargo xtask licenses --destination /path/to/output`; run `cargo fetch --locked` first, since generation is offline. MPL sources are included as `.crate` archives (gzip-compressed tar).

Desktop CI generates this bundle once on Linux and shares it across all OS builds with `cargo xtask package --licenses-dir "$RUNNER_TEMP/licenses"`. A single `actions/cache` step restores and saves it in the runner temporary directory, separate from the Rust build cache. The key covers `Cargo.lock`, workspace manifests, `about.toml`, license texts, and `xtask/src/licenses.rs`; workflow changes and unrelated xtask code do not invalidate it. Manifests and license inputs are included because they can change the notices without changing the lockfile. Release version changes happen before computing the key. Caches use GitHub's branch scoping, with main caches also available to PRs. When reusing a bundle locally, regenerate it after these inputs change. Without `--licenses-dir`, packaging generates fresh notices as before.

Notices are in `licenses/` in Windows ZIPs, `Contents/Resources/licenses/` in macOS apps, `usr/lib/irodoru/licenses/` in Linux packages, and `/app/licenses/` in Docker. `THIRD-PARTY-NOTICES.txt` contains license texts; `THIRD-PARTY-SOURCES.txt` identifies versions and source availability. The generator only replaces empty directories or previously generated bundles.

When updating dependencies, review `about.toml` and the generated notices. If a crate lacks original copyright notices, investigate the upstream source and add a version-specific entry to `licenses/clarifications.json`. Native ONNX Runtime and DirectML are reviewed separately; updating `ort-sys` requires reviewing `licenses/native/`. The optional OpenVINO GPU image preserves notices from the installed wheels.

Models are downloaded from reviewed revisions in `licenses/model/revisions.json`. Review model cards and ancestor licenses before updating these pins, then update notices and re-export. Each exported model and codec includes `LICENSES.txt` and `provenance.json`, which packaging and downloads require. Re-export old model directories that lack these files before distribution.

Release flow:

1. Push a tag such as `1.2.3`. The release workflow runs the app and Docker workflows in parallel.
2. The app workflow publishes models to GHCR and builds apps with the distribution manifest. The Docker workflow builds and publishes its image independently.
3. After both succeed, the final `release` job publishes a GitHub Release with OS packages. Docker usage is documented in [Docker.md](Docker.md). Use the SHA-256 digest shown by GitHub for each asset to verify downloads; separate checksum files are not generated.

App assets use `irodoru_<version>_<os>_<arch>.<extension>`, for example `irodoru_0.0.4_windows_x64.zip`, `irodoru_0.0.4_macos_arm64.dmg`, and `irodoru_0.0.4_linux_x64.deb` or `.AppImage`. The version comes from the release tag, including prerelease suffixes. Split downloads append `.part001`, `.part002`, etc. Prepare these names with `cargo xtask release app TAG TARGET DEST`.

Release notes are generated by GitHub using `.github/release.yml`. Tags such as `1.2.3-beta.1` create prereleases. Releases stay in draft during upload; rerun CI after a failure. Regular main and PR builds do not create releases.

GHCR packages:

- `irodoru`: Docker images under version tags and CI caches under `cache-amd64` and `cache-arm64`. Only main CI updates caches.
- `irodoru-models`: Model ZIPs for desktop apps. If the first public-access check fails, make this package public and rerun CI. Keep models referenced by released apps.
- `irodoru-ci-cache`: Unused legacy cache. It can be removed after main CI creates the replacement caches.

To check package contents (required licenses and absence of model weights):

```bash
bash scripts/check-package.sh TARGET
```

Replace `TARGET` with the Rust target. This check does not load models or run synthesis. Run `cargo xtask help` for other commands.

## Windows ML runtime

Windows builds require Windows 11 24H2 or later. CPU inference needs no separate runtime installation. GPU/NPU acceleration uses the x64 Windows App SDK Runtime **2.6.5 or later, major version 2**. Install the official runtime, or run the reviewed installer script from PowerShell (also used by CI):

```powershell
./scripts/install-windows-ml.ps1
```

The script downloads the pinned official NuGet package, verifies its SHA-256, and installs its signed framework and companion MSIX packages for the current user. It does not add these binaries to the app ZIP.

Before building on Windows, run the pinned CPU runtime build (Visual Studio C++ tools, Python, CMake and Git required):

```powershell
./scripts/build-cpu-ort.ps1
$env:IRODORU_CPU_ORT_LIB_DIR = (Resolve-Path target/cpu-ort/lib).Path
```

This builds ONNX Runtime 1.31.0 with `/MT`, no accelerator DLL dependencies, and full ONNX operator support. CI caches the static library. Its linked executable size increase must be measured from Windows release artifacts; the earlier 20–40 MiB figure was an estimate.

At startup the engine selects one ORT API for the entire process. If the shared package cannot be loaded or does not support API 27, it selects the statically linked CPU API. It never mixes handles from two runtimes. With Windows ML available, even CPU fallback sessions use that selected shared runtime.

The engine adds `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe` to the process package dependency graph using the Windows dynamic-dependency APIs. It loads ONNX Runtime and the Windows ML catalog from that resolved package's absolute directory. Rust `ort` uses API version 27, matching the reviewed Microsoft.Windows.AI.MachineLearning 2.4.89 headers, rather than requiring the bundled runtime to expose API 31.

Windows ML prepares certified execution providers and their dependencies; the engine registers their library paths in its own ORT environment. Windows defaults to automatic GPU/NPU selection. Original ONNX graphs and the custom Snake operator remain in use, with CPU graph fallback for automatic mode. Explicit CPU selection suppresses automatic EP selection. Explicit GPU/NPU requests fail if no matching device is available or no session can be created; individual unsupported operators may still run on CPU. Non-Windows backend behavior remains unchanged. The catalog can download providers on first use; deployment in an offline environment requires preparing them beforehand.

Validate the staged executable with MSVC tools installed:

```powershell
./scripts/check-windows-runtime.ps1 -Executable target/x86_64-pc-windows-msvc/release/native/irodoru.exe
# On a machine with a compatible hardware GPU (may download an EP):
./scripts/check-windows-runtime.ps1 -Executable target/x86_64-pc-windows-msvc/release/native/irodoru.exe -Gpu
```

The check rejects direct ONNX Runtime, DirectML, and MSVC runtime imports, then launches a temporary copy of only the executable with a system-only PATH. The application reports the framework package or embedded CPU runtime used. `-EmbeddedCpu` forces the embedded path in a fresh diagnostic process even if Windows ML is installed. CI checks that path before installing Windows ML, then checks the shared runtime, with a tiny CPU Add inference in each; GPU mode additionally executes a tiny Add graph with CPU fallback disabled. Hardware GPU/NPU execution, full speech synthesis and offline reuse must still be tested on Windows; a successful loader check is not evidence of those checks.

References: [Windows ML deployment](https://learn.microsoft.com/windows/ai/new-windows-ml/distributing-your-app), [provider registration](https://learn.microsoft.com/windows/ai/new-windows-ml/register-execution-providers), [reviewed runtime package](https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime/2.6.5).

The footer lists active providers. Model settings show each loaded graph’s selected provider and CPU fallback state. This is session selection, not per-operator profiling; an accelerator session may execute unsupported operators on CPU. Execution failures update the live status when the graph switches to CPU.

## macOS Core ML

macOS builds enable the `ort/coreml` feature and use the reviewed ORT 1.31.0
Apple Silicon archive with Core ML support. The OS supplies the Core ML framework;
users need no additional runtime. The existing macOS 15 minimum is unchanged.

The app defaults to automatic selection. Sessions register Core ML with MLProgram
format and all compute units enabled. Original ONNX graphs and the Snake custom
operator remain supported through CPU partitioning. Provider registration errors
are propagated to the engine so automatic mode can rebuild a CPU-only session;
execution failures use the same graph fallback and live provider tracker as Windows.
Explicit CPU selection never registers Core ML. GPU/NPU options select
CPUAndGPU/CPUAndNeuralEngine policies, not guaranteed hardware placement. The
entries in device discovery describe these policies. Explicit OpenVINO overrides
remain available.

The UI reports `Core ML（自動選択）` or the selected compute policy. Core ML chooses
the actual hardware per operation, so this label does not prove GPU or Neural
Engine execution. Compiled model caching is not enabled to avoid stale weights
when importing replacement models. Mac CI checks the native code and package contents. Full speech quality, latency
and actual accelerator use still require validation on a Mac.

Reference: [Core ML execution provider](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html).

## CI build caches

The app workflow saves caches on pull requests as well as `main`. New commits
wait for the active run to finish and save its caches instead of cancelling an
expensive build; GitHub retains only the newest pending run in the group:

- Rust uses sccache's GitHub Actions backend to reuse compilation results by
  content, with incremental compilation disabled. Cargo registry downloads are
  cached separately; full target trees are not duplicated in that cache.
- The prepared model bundle is saved immediately after export. Only model
  licenses, exporter dependencies and bundle preparation code invalidate it;
  changing a native-library notice does not export the models again.
- Windows' embedded CPU ONNX Runtime archive is keyed by its pinned build script
  and saved before application checks, so a later test failure does not discard
  the expensive native build.
- `cargo-about` and `cargo-packager` are cached by exact version, features, OS and
  architecture, immediately after installation.
- Completed desktop packages are reused only on an exact match of source,
  manifests, resources (including the downloaded model distribution manifest),
  licenses, build scripts, workflow configuration, Rust compiler and runner image.
  Release versions are applied before computing the key. A hit skips release
  compilation and packaging; tests, package inspection and Windows runtime
  probes still run. README-only edits do not invalidate packages.

Changed inputs rebuild the affected layer. GitHub's cache scope still applies:
PR caches can be reused by that PR, while `main` cannot read a PR's private cache.
Cache eviction, a new runner image or compiler, and a first run on a new branch
can therefore require a fresh build. Check the restore steps and sccache's final
statistics when investigating a slow run.

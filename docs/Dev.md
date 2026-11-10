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

Development builds compile dependencies with optimizations so the UI stays responsive; the first build takes longer.

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

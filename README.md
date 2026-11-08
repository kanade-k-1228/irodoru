# irodoru

A speech app and Docker inference server built on [Irodori-TTS](https://github.com/Aratako/Irodori-TTS).
Both use [Irodori-TTS-v4.1-Anime](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime) and its codec (about 3.44 GB).
The app downloads these models on first launch and then runs offline. It targets Windows,
macOS (Apple Silicon), Linux, iOS, and Android. Docker images include the models.

Download app packages from [Releases](https://github.com/kanade-k-1228/irodoru/releases).
App packages contain no model weights. On Windows, extract the ZIP and run `irodoru.exe`.
The first launch downloads one ZIP containing all models, verifies its SHA-256, and extracts
and verifies the model files in its application data directory. Setup temporarily needs about
twice the model size on disk (the ZIP plus extracted files); the ZIP is removed after setup.
Subsequent app updates reuse the downloaded models when their contents have not changed.
A failed download restarts the ZIP download. If extraction fails after the ZIP was verified,
retrying reuses the downloaded ZIP.

Models are shared by all app platforms through the public GHCR package
`ghcr.io/kanade-k-1228/irodoru-models`. The app downloads them directly over HTTPS;
no Docker installation or GitHub account is needed. Its embedded manifest pins the model
ZIP by SHA-256, independently of app releases. Keep published model versions available in GHCR
so older apps can still download them. If an app package itself is split, follow its included
README to join it.

## Projects

Create or open an `.iro` project from the app header. Each project contains its own script,
characters, reference audio, icons, and generated audio; there is no shared character or
voice library. Existing pre-project application data is not migrated.

The `.iro` file is a ZIP archive internally. Existing `.zip` projects can still be
opened and saved at their original paths; new projects default to `.iro`.

Desktop builds can open `.iro` files from the OS, including while the app is running.
The current project is saved before switching; requests wait while synthesis or
character editing is in progress. Multiple files open in arrival order, leaving the
last project visible. Failed opens leave the current project intact.

On Windows, the release ZIP is portable: right-click an `.iro` file, choose **Open
with → Choose another app**, select `irodoru.exe`, and choose to always use it for
`.iro` files. macOS app bundles and Linux packages include file association metadata;
registration depends on installing the app/package and the desktop environment.
On Linux, install the `.deb` package, or enable desktop integration for an AppImage.
Existing default app choices are not forcibly changed. Mobile file associations
are not enabled yet; use the in-app project picker on iOS and Android.

```text
project.iro
├── project.yaml                      # format version, script, generation settings
├── characters/<character-id>/
│   ├── character.yaml                # character settings; relative asset paths
│   ├── reference.wav                 # optional, copied into this character
│   ├── icon.png                      # optional
│   └── previews/{audio.wav,meta.yaml} # latest sample committed with the character
└── voices/<take-id>/{audio.wav,meta.yaml}
```

Character directory IDs stay stable when display names change. All asset references are
relative, so moving or renaming the project file does not break them. Regeneration replaces the
previous audio for that script line only after a successful generation. Deleting a line
also removes its audio; deleting a character keeps already-generated script audio.

Edits and committed character changes trigger autosave on a dedicated `project-save`
thread. Action bursts are combined for 200 ms (at most one second of continuous changes),
and changes during saving schedule another save. WAV and PNG files are stored without
ZIP compression; YAML is deflated. The saver pins a consistent set of working files with temporary hard links (or copies
when hard links are unavailable), releases the lock, writes and syncs a new ZIP beside the destination,
and replaces the previous ZIP only on success. Closing or switching projects waits for
pending saves; errors remain visible and the Save button retries.

Working files and uncommitted character samples use a separate temporary directory for
each open project. They are not a shared library or a recovery history. Save or cancel a
character edit before switching projects. Project dialogs currently require a local
filesystem path; document-provider URLs such as Android `content://` are not supported.

## App development

Requires Rust, Node.js 26, pnpm, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
The app can be built without models. Local inference uses an exported model directory during
development; model export requires Python and uv.

```bash
pnpm install --frozen-lockfile
pnpm models:export    # Export anime and codec
pnpm models:prepare   # Stage models for local development (excluded from app packages)
pnpm dev
pnpm build:app
```

For existing models, replace export and prepare with `pnpm models:prepare --source /path/to/models`.
For mobile, run `pnpm android:init` then `pnpm android:build`, or `pnpm ios:init` then `pnpm ios:build`.

Release CI exports the models once and uses Python's standard library to create one uncompressed
ZIP64 archive with deterministic timestamps, permissions, and entry order. It publishes the ZIP
as one OCI layer to `ghcr.io/<owner>/<repository>-models`, then checks anonymous access to the
exact manifest digest and archive blob descriptor. Archives larger than GHCR's 10 GB layer
limit are rejected.
Each app embeds only the small download manifest. Model publication runs before app builds;
app packages are attached to the GitHub Release only after all checks and builds succeed.
The ZIP's SHA-256 is its GHCR tag, so changing the archive format keeps older downloads available.
The model content hash remains the local cache revision, independent of packaging. Identical
archives reuse the same blob and deterministic OCI metadata across app releases.

On the first release, GitHub creates the model package with **private** visibility. The public
access check intentionally stops the workflow: open the package's **Package settings → Change
visibility → Public**, then rerun the failed workflow. The package is linked to this repository
and CI publishes with `GITHUB_TOKEN` and `packages: write`; no long-lived token is required.
See [GitHub's package visibility documentation](https://docs.github.com/en/packages/learn-github-packages/configuring-a-packages-access-control-and-visibility).

Main/PR builds do not publish or require GHCR credentials. Their manifests have no download
repository: they can use an existing matching model cache, but cannot download unpublished
models. Source builds use the checked-in placeholder manifest unless a generated
`model-distribution.json` is placed in `pkgs/app/src-tauri/resources/`. Desktop development can
use `pnpm models:prepare` as above.

## Docker server

Run a release image using its version tag:

```bash
docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/kanade-k-1228/irodoru:0.0.1
```

Generate Japanese speech:

```bash
curl http://127.0.0.1:8080/ \
  -H 'content-type: application/json' \
  -d '{"text":"こんにちは","caption":"落ち着いた声","seed":1}' -o out.wav
```

See [OpenAPI](pkgs/server/openapi.json) for the API. Authentication is not implemented.
To build locally, run `docker compose up --build -d`.

To use another model, export and mount it yourself (small model example):

```bash
uv run --project pkgs/engine/script python pkgs/engine/script/export_onnx.py --out models --models small
docker run --rm -p 127.0.0.1:8080:8080 \
  -v "$PWD/models/small:/models/model:ro" \
  -v "$PWD/models/codec:/models/codec:ro" \
  ghcr.io/kanade-k-1228/irodoru:0.0.1
```

## Development commands

Run checks with `pnpm verify`. Generate the API schema and types with `pnpm generate`.
See [package.json](package.json) for other commands and [CI](.github/workflows) for builds and releases.

## License

[MIT](LICENSE), like the model. Dependencies and models retain their own licenses.
Model use is also subject to the upstream [usage restrictions](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions).

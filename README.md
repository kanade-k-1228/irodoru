# irodoru

A speech app and Docker inference server built on [Irodori-TTS](https://github.com/Aratako/Irodori-TTS).
Both include [Irodori-TTS-v4.1-Anime](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime) and its codec (about 3.44 GB).
The app runs offline and targets Windows, macOS (Apple Silicon), Linux, iOS, and Android.

Download app packages from [Releases](https://github.com/kanade-k-1228/irodoru/releases).
Large packages are split into parts; download all parts and follow the included README to join and verify them.
On Windows, extract the joined ZIP and run `irodoru.exe`.

## Projects

Create or open a project ZIP from the app header. Each project contains its own script,
characters, reference audio, icons, and generated audio; there is no shared character or
voice library. Existing pre-project application data is not migrated.

```text
project.zip
├── project.yaml                      # format version, script, generation settings
├── characters/<character-id>/
│   ├── character.yaml                # character settings; relative asset paths
│   ├── reference.wav                 # optional, copied into this character
│   ├── icon.png                      # optional
│   └── previews/{audio.wav,meta.yaml} # latest sample committed with the character
└── voices/<take-id>/{audio.wav,meta.yaml}
```

Character directory IDs stay stable when display names change. All asset references are
relative, so moving or renaming the ZIP does not break them. Regeneration replaces the
previous audio for that script line only after a successful generation. Deleting a line
also removes its audio; deleting a character keeps already-generated script audio.

Edits and committed character changes trigger autosave on a dedicated `project-save`
thread. Action bursts are combined for 200 ms (at most one second of continuous changes),
and changes during saving schedule another save. WAV and PNG files are stored without
ZIP compression; YAML is deflated. The saver opens a consistent set of working files
under a short lock, releases the lock, writes and syncs a new ZIP beside the destination,
and replaces the previous ZIP only on success. Closing or switching projects waits for
pending saves; errors remain visible and the Save button retries.

Working files and uncommitted character samples use a separate temporary directory for
each open project. They are not a shared library or a recovery history. Save or cancel a
character edit before switching projects. Project dialogs currently require a local
filesystem path; document-provider URLs such as Android `content://` are not supported.

## App development

Requires Rust, Node.js 26, pnpm, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
Model export also requires Python and uv.

```bash
pnpm install --frozen-lockfile
pnpm models:export    # Export anime and codec
pnpm models:prepare   # Copy models into app resources
pnpm dev
pnpm build:app
```

For existing models, replace export and prepare with `pnpm models:prepare --source /path/to/models`.
For mobile, run `pnpm android:init` then `pnpm android:build`, or `pnpm ios:init` then `pnpm ios:build`.

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

[MIT](LICENSE), like the bundled model. Dependencies and models retain their own licenses.
Model use is also subject to the upstream [usage restrictions](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions).

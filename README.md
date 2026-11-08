# irodoru

A speech app and Docker inference server built on [Irodori-TTS](https://github.com/Aratako/Irodori-TTS).
Both include [Irodori-TTS-v4.1-Anime](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime) and its codec (about 3.44 GB).
The Rust/GPUI desktop app runs offline on Windows x64, macOS 15+ (Apple Silicon), and Linux x64.

Download app packages from [Releases](https://github.com/kanade-k-1228/irodoru/releases).
Large packages are split into parts; download all parts and follow the included README to join and verify them.
On Windows, extract the joined ZIP and run `irodoru.exe`.

See [使い方ガイド（日本語・スクリーンショット付き）](docs/user-guide.md) to get started with the desktop app.

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
ZIP compression; YAML is deflated. The saver pins a consistent set of working files with temporary hard links (or copies
when hard links are unavailable), releases the lock, writes and syncs a new ZIP beside the destination,
and replaces the previous ZIP only on success. Closing or switching projects waits for
pending saves; errors remain visible and the Save button retries.

Working files and uncommitted character samples use a separate temporary directory for
each open project. They are not a shared library or a recovery history. Save or cancel a
character edit before switching projects. Project dialogs currently require a local
filesystem path; document-provider URLs such as Android `content://` are not supported.

## App development

See [the app architecture guide](pkgs/app/README.md) for module boundaries, state flow and extension points.

The app and development tasks use Cargo; Node.js and pnpm are not required. Requires stable Rust and the [GPUI Kit native prerequisites](https://gpui-kit.com/docs/installation/).
Linux needs a graphical Wayland or X11 session with Vulkan, plus ALSA, fontconfig and XKB development libraries.
Native file dialogs require `xdg-desktop-portal` and a portal backend for the desktop (GTK, GNOME or KDE).
Packaging additionally needs `cargo install cargo-packager --version 0.11.8 --locked`, `patchelf` on Linux, or 7-Zip on Windows.
Model export also requires Python and uv.

```bash
cargo xtask models export    # Export anime and codec
cargo xtask models prepare   # Copy models into app resources
cargo xtask dev
cargo xtask package
```

For existing models, replace export and prepare with `cargo xtask models prepare --source /path/to/models`.
The app UI and runtime are entirely Rust; Android and iOS builds are no longer provided.
Reference audio supports WAV, MP3, FLAC, Ogg Vorbis and AAC/M4A; imports become 48 kHz mono WAV, up to 60 seconds (minimum 1 second).
Icons support PNG, JPEG, WebP, GIF, BMP, TIFF and SVG up to 10 MB, cropped to a 128-pixel square.
The top-left irodoru menu contains project actions, theme, preferences and app information. Ctrl/Cmd+Shift+N opens an independent window; Ctrl/Cmd+N/O/S create, open and save projects, Ctrl/Cmd+, opens preferences, and Ctrl/Cmd+W closes the current window after saving. The footer displays the project archive path. A project cannot be opened in two windows at once.
Theme preferences use confy with YAML: `theme: light` or `theme: dark` in `config.yaml` in the OS app configuration directory. The default is dark; old JSON settings are not read. Existing project ZIPs remain compatible.
For development, `IRODORU_MODELS` can point to a directory containing `anime/` and `codec/`.
`cargo xtask package --no-bundle` creates a standalone directory with models.
Run `cargo clippy --locked -p irodoru-desktop --features desktop --all-targets -- -D warnings` to check the native UI after installing the platform dependencies.
CI builds all three desktop targets and runs offline synthesis from the extracted ZIP, DMG, AppImage and deb packages. Run `bash scripts/smoke-package.sh TARGET` to repeat this check locally after packaging, or launch the shipped executable with `--smoke-test`.

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

Run checks with `cargo xtask verify`. Generate the OpenAPI schema with `cargo xtask generate`.
Run `cargo xtask help` for other commands and see [CI](.github/workflows) for builds and releases.

## License

[MIT](LICENSE), like the bundled model. Dependencies and models retain their own licenses.
Model use is also subject to the upstream [usage restrictions](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions).

# irodoru

A speech app and Docker inference server built on [Irodori-TTS](https://github.com/Aratako/Irodori-TTS).
Both include [Irodori-TTS-v4.1-Anime](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime) and its codec (about 3.44 GB).
The app runs offline and targets Windows, macOS, Linux, iOS, and Android.

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

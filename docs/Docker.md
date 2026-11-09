# Docker guide

[README](../README.md) · [User guide](Usage.md) · [Development](Dev.md)

The Docker image runs an HTTP speech server with bundled models. Install Docker and use a version tag from [Releases](https://github.com/kanade-k-1228/irodoru/releases). These examples use `0.0.1`.

## Run and generate audio

```bash
docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/kanade-k-1228/irodoru:0.0.1
```

Wait for the models to load, then send a request from another terminal:

```bash
curl http://127.0.0.1:8080/ \
  -H 'content-type: application/json' \
  -d '{"text":"Hello","caption":"A calm voice","seed":1}' \
  -o out.wav
```

- Output is a 48 kHz mono WAV. See [OpenAPI](../pkgs/server/openapi.json) for request parameters.
- The server has no authentication. The example binds it to localhost.
- Press `Ctrl+C` in the server terminal to stop it.
- To select the CPU and thread count, append `serve --device cpu --threads 4` after the image name.

## Use another model

With the repository, Python, and uv installed, export a model and mount it read-only. This example uses the small model:

```bash
uv run --project pkgs/engine/script python pkgs/engine/script/export_onnx.py --out models --models small
docker run --rm -p 127.0.0.1:8080:8080 \
  -v "$PWD/models/small:/models/model:ro" \
  -v "$PWD/models/codec:/models/codec:ro" \
  ghcr.io/kanade-k-1228/irodoru:0.0.1
```

## Build locally

Run from the repository root. The first build also downloads and converts models.

```bash
docker compose up --build -d
docker compose logs -f backend
# Stop the server
docker compose down
```

- To use existing `models/anime/` and `models/codec/`, run `docker compose --profile local up --build -d local`. Stop it with `docker compose --profile local down`.
- For an Intel GPU on WSL2, use the `wsl-gpu` profile and service. This requires a Windows GPU driver and `/dev/dxg`.
- Run one service at a time: they share the same port.

## Cancel a request

Abort the HTTP request or close its connection to cancel synthesis; no separate
cancel endpoint is needed. For example, pass an `AbortController` signal to
`fetch` and call `controller.abort()` when a newer navigation instruction arrives.

Once the server detects the disconnect, a request waiting for the engine is
discarded. Running synthesis stops at the next cancellation checkpoint, including
each sampling step and before audio decoding. An in-progress model operation
must finish first; it is not forcibly interrupted. Other requests can then use
the engine normally.

If you use a reverse proxy, it must propagate client disconnects to the upstream
server. Cancellation begins when the server observes the disconnect, not when
the client locally stops waiting for the response.

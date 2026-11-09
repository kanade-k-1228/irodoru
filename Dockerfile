# syntax=docker/dockerfile:1.7

# ---- 1. Export Irodori-TTS checkpoints to ONNX (architecture-independent) ----
FROM --platform=$BUILDPLATFORM python:3.10-slim AS models
RUN apt-get update \
    && apt-get install -y --no-install-recommends git build-essential libsndfile1 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=ghcr.io/astral-sh/uv:0.13.7 /uv /usr/local/bin/uv
WORKDIR /export
COPY pkgs/engine/script/pyproject.toml pkgs/engine/script/uv.lock pkgs/engine/script/.python-version ./
RUN --mount=type=cache,target=/root/.cache/uv uv sync --frozen
COPY pkgs/engine/script/*.py ./
COPY licenses /licenses
RUN --mount=type=cache,target=/root/.cache/huggingface \
    uv run --no-sync python -u export_onnx.py --out /models --models anime

# ---- 2. Rust server ----
FROM rust:1-trixie AS server
WORKDIR /src
RUN cargo install cargo-about --version 0.9.3 --locked --features cli
COPY Cargo.toml Cargo.lock ./
COPY LICENSE about.toml ./
COPY licenses ./licenses
COPY pkgs/engine ./pkgs/engine
COPY pkgs/model-bundle ./pkgs/model-bundle
COPY pkgs/server ./pkgs/server
# The UI needs a manifest and placeholder targets for license resolution.
# xtask generates notices and corresponding MPL sources for the runtime image.
COPY pkgs/app/Cargo.toml ./pkgs/app/Cargo.toml
COPY xtask ./xtask
COPY pkgs/app/icons/ui/LICENSE ./pkgs/app/icons/ui/LICENSE
RUN mkdir -p pkgs/app/src \
    && touch pkgs/app/src/lib.rs pkgs/app/src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p irodori-server \
    && mkdir -p /out \
    && cp target/release/irodori-server /out/ \
    && (cp target/release/*.so* /out/ 2>/dev/null || true) \
    && cargo fetch --locked \
    && cargo run --locked --package xtask -- licenses --destination /out/licenses

# ---- 3. API runtime: loads /models/model and /models/codec; mount them to use other models ----
FROM debian:trixie-slim AS backend-runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends libstdc++6 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app irodori
COPY --from=server /out/ /app/
ENV LD_LIBRARY_PATH=/app
USER irodori
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=180s \
    CMD ["/app/irodori-server", "ready"]
ENTRYPOINT ["/app/irodori-server"]
CMD ["serve"]

# ---- Optional Intel GPU/NPU runtime (CPU images do not depend on OpenVINO) ----
FROM python:3.12-slim AS openvino
RUN pip install --no-cache-dir --target /opt/ov 'openvino==2026.4.1' \
    && ln -s libopenvino_c.so.2641 /opt/ov/openvino/libs/libopenvino_c.so
COPY scripts/collect-wheel-licenses.py /collect-wheel-licenses.py
RUN python /collect-wheel-licenses.py /opt/ov /opt/ov-licenses

FROM ubuntu:26.04 AS backend-gpu-runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends libstdc++6 ca-certificates \
       intel-opencl-icd libze-intel-gpu1 libze1 ocl-icd-libopencl1 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app irodori
COPY --from=server /out/ /app/
COPY --from=openvino /opt/ov/openvino/libs/ /app/runtime/openvino/
COPY --from=openvino /opt/ov-licenses/ /app/licenses/openvino/
ENV LD_LIBRARY_PATH=/app:/usr/lib/wsl/lib
USER irodori
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=300s \
    CMD ["/app/irodori-server", "ready"]
ENTRYPOINT ["/app/irodori-server"]
CMD ["serve"]

# ---- 4. Production API with models baked in ----
FROM backend-runtime AS backend
COPY --from=models /models/anime /models/model
COPY --from=models /models/codec /models/codec

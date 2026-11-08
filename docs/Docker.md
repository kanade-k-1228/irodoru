# Dockerイメージの使い方

[README](../README.md) · [アプリの使い方](Usage.md) · [開発](Dev.md)

モデルを同梱したHTTP音声合成サーバーです。Dockerを用意し、[Releases](https://github.com/kanade-k-1228/irodoru/releases) に対応するバージョンタグを指定します。以下は `0.0.1` の例です。

## 起動・音声生成

```bash
docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/kanade-k-1228/irodoru:0.0.1
```

モデルの読み込み後、別のターミナルからリクエストします。

```bash
curl http://127.0.0.1:8080/ \
  -H 'content-type: application/json' \
  -d '{"text":"こんにちは","caption":"落ち着いた声","seed":1}' \
  -o out.wav
```

48 kHz・モノラルのWAVが保存されます。パラメーターの詳細は [OpenAPI](../pkgs/server/openapi.json) を参照してください。認証機能はないため、上の例では接続先をローカルに限定しています。停止は起動したターミナルで `Ctrl+C` です。

CPUを明示してスレッド数を指定する場合は、イメージ名の後ろに `serve --device cpu --threads 4` を付けます。

## 別のモデルを使う

リポジトリとPython・uvを用意し、モデルをエクスポートして読み取り専用でマウントします。以下はsmallモデルの例です。

```bash
uv run --project pkgs/engine/script python pkgs/engine/script/export_onnx.py --out models --models small
docker run --rm -p 127.0.0.1:8080:8080 \
  -v "$PWD/models/small:/models/model:ro" \
  -v "$PWD/models/codec:/models/codec:ro" \
  ghcr.io/kanade-k-1228/irodoru:0.0.1
```

## ローカルでビルド

リポジトリのルートで実行します。初回はモデルの取得・変換も行います。

```bash
docker compose up --build -d
docker compose logs -f backend
# 停止
docker compose down
```

既存の `models/anime/` と `models/codec/` を使う場合は `docker compose --profile local up --build -d local`、停止は `docker compose --profile local down` です。WSL2のIntel GPU向けには `wsl-gpu` プロファイルと同名サービスがあり、Windows側のGPUドライバーと `/dev/dxg` が必要です。各サービスは同じポートを使うため、1つずつ起動してください。

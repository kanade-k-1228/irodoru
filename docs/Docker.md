# Docker Server Usage

Docker版は、モデルを同梱したHTTP音声合成サーバーです。Dockerをインストールし、[リリース](https://github.com/kanade-k-1228/irodoru/releases)のバージョンタグを指定してください。以下は `0.0.6` の例です。

## 起動して音声を生成する

```bash
docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/kanade-k-1228/irodoru:0.0.6
```

モデルの読み込みを待ち、別のターミナルからリクエストを送ります。

```bash
curl http://127.0.0.1:8080/ \
  -H 'content-type: application/json' \
  -d '{"text":"こんにちは。今日はいい天気ですね。","caption":"落ち着いた声","seed":1}' \
  -o out.wav
```

- 出力は48 kHz・モノラルのWAVです。指定できるパラメーターは [OpenAPI](../pkgs/server/openapi.json) を参照してください。
- サーバーに認証機能はありません。この例ではローカルホストだけに公開します。
- サーバー側のターミナルで `Ctrl+C` を押すと停止します。
- CPUとスレッド数を指定する場合は、イメージ名の後に `serve --device cpu --threads 4` を追加します。

## 別のモデルを使う

このリポジトリ、Python、uvを用意し、モデルを変換して読み取り専用でマウントします。以下はsmallモデルの例です。

```bash
uv run --project pkgs/engine/script python pkgs/engine/script/export_onnx.py --out models --models small
docker run --rm -p 127.0.0.1:8080:8080 \
  -v "$PWD/models/small:/models/model:ro" \
  -v "$PWD/models/codec:/models/codec:ro" \
  ghcr.io/kanade-k-1228/irodoru:0.0.6
```

## ローカルでビルドする

リポジトリのルートで実行します。初回はモデルのダウンロードと変換も行います。

```bash
docker compose up --build -d
docker compose logs -f backend
# サーバーを停止
docker compose down
```

- 手元の `models/anime/` と `models/codec/` を使う場合は、`docker compose --profile local up --build -d local` を実行します。停止は `docker compose --profile local down` です。
- WSL2でIntel GPUを使う場合は、`wsl-gpu` プロファイルと同名サービスを指定します。Windows側のGPUドライバーと `/dev/dxg` が必要です。
- 各サービスは同じポートを使うため、同時には起動しないでください。

## リクエストをキャンセルする

HTTPリクエストを中断するか接続を閉じると、音声生成をキャンセルできます。専用のキャンセルAPIは不要です。たとえば `fetch` に `AbortController` のシグナルを渡し、不要になった時点で `controller.abort()` を呼び出します。

サーバーが切断を検知すると、推論待ちのリクエストは破棄されます。実行中の生成は、次のサンプリングステップや音声デコード前などのキャンセル確認箇所で停止します。実行中のモデル演算自体は中断せず、その完了を待ちます。その後、別のリクエストがエンジンを利用できます。

リバースプロキシを使う場合は、クライアントの切断が上流サーバーへ伝わる設定にしてください。キャンセルが始まるのは、クライアントが応答を待つのをやめた時点ではなく、サーバーが切断を検知した時点です。

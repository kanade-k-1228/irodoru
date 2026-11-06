# irodoru

[Irodori-TTS](https://github.com/Aratako/Irodori-TTS) を使う音声合成アプリと推論サーバ。
Rust の共通 Engine がモデルとキャッシュを持ち、両方から利用する。

- **アプリ**: Tauri 製。anime と codec を同梱し、オフラインで生成する。台本・キャラクター・音声はローカルに保存する。
- **サーバ**: Docker で HTTP API を提供する。配布イメージは anime と codec を同梱する。

同梱モデルは [Irodori-TTS-v4.1-Anime（Hugging Face）](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime)。
アプリの対象は Windows・macOS・Linux・iOS・Android。同梱モデルは約 3.44 GB。

## 構成

```text
pkgs/engine/  推論エンジン
  script/     モデル変換と数値検証スクリプト
pkgs/server/  HTTP サーバ
pkgs/app/     Tauri アプリ（React + Rust）
scripts/      モデル同梱・型生成・配布の補助処理
```

## アプリの開発

Rust、Node.js 26、pnpm と [Tauri の開発依存関係](https://v2.tauri.app/start/prerequisites/) が必要。
モデルを変換する場合は Python と uv も使う。

```bash
pnpm install --frozen-lockfile
pnpm models:export             # anime と codec を models/ に変換
pnpm models:prepare            # アプリ用リソースへ配置
pnpm dev                       # 開発起動
pnpm build:app                 # 現在の OS 向けにビルド
```

変換済みモデルがある場合は、export・prepare の代わりに次を実行する。モデル更新時も再配置する。

```bash
pnpm models:prepare --source /absolute/path/to/models
```

Android は `pnpm android:init` → `pnpm android:build`、iOS は `pnpm ios:init` → `pnpm ios:build`。
Android SDK・NDK・JDK、または macOS・Xcode が必要。ストア配布用の署名は別途設定する。

## サーバ

リリースイメージのタグは `0.0.1` のようなバージョン番号のみ。以下は `0.0.1` を使う例。

```bash
docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/kanade-k-1228/irodoru:0.0.1
```

別モデルを使う場合は利用者側で変換し、モデルと codec をマウントする（small の例）。

```bash
uv run --project pkgs/engine/script python pkgs/engine/script/export_onnx.py --out models --models small
docker run --rm -p 127.0.0.1:8080:8080 \
  -v "$PWD/models/small:/models/model:ro" \
  -v "$PWD/models/codec:/models/codec:ro" \
  ghcr.io/kanade-k-1228/irodoru:0.0.1
```

ローカルでビルドする場合:

```bash
docker compose up --build -d
# 変換済みの models/ を使う場合
IRODORI_MODEL=small docker compose up --build -d local
```

停止は `docker compose down`。`local` / `wsl-gpu` は `docker compose down local` のようにサービス名を指定する。
各サービスは同じポートを使うため、1つずつ起動する。

既定の公開先は `127.0.0.1:8080`。API 認証は実装していない。
アプリはこのサーバへの接続を必要としない。

```bash
curl http://127.0.0.1:8080/ \
  -H 'content-type: application/json' \
  -d '{"text":"こんにちは","caption":"落ち着いた声","seed":1}' -o out.wav
```

`POST /` は WAV を返す。seed などの生成条件は WAV の `LIST/INFO/ICMT` チャンクに JSON で入る。
リクエストの詳細は [OpenAPI](pkgs/server/openapi.json) を参照。

Docker を使わずに起動する場合:

```bash
pnpm build:server
target/release/irodori-server serve --model models/anime --codec models/codec
```

サーバ設定は CLI 引数で指定する。詳細は `irodori-server serve --help`。
CPU は ONNX Runtime、GPU/NPU は OpenVINO を使い、`--device` で選択できる。

WSL2 の Intel GPU を使う場合:

```bash
scripts/setup-openvino.sh --wsl
.runtime/setup-venv/bin/python pkgs/engine/script/portable_onnx.py --model-dir models
docker compose up --build -d wsl-gpu
```

## 検証・生成

Docker イメージは main 更新時にビルドを検証し、GitHub Release 公開時だけ GHCR に push する。

```bash
pnpm verify                    # 通常の検証一式（モデル不要）
pnpm verify:app                # Tauri のネイティブ機能を含む Clippy
pnpm verify:export             # Python の数値検証
pnpm generate                  # OpenAPI・TypeScript 型を再生成
```

その他のコマンドは [package.json](package.json)、ビルド・配布設定は [CI](.github/workflows) を参照。

## ライセンス

本リポジトリは同梱モデルと同じ [MIT License](LICENSE)。依存ライブラリ・モデルにはそれぞれのライセンスが適用される。
モデルの利用には、公開元が定める [利用上の制約](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions) も適用される。
本人の明示的な同意のない声のクローン・なりすましや、誤認・偽情報を目的とする音声生成は禁止されている。
生成された声が偶然実在人物に似る場合があり、生成物の利用と法令遵守は利用者の責任となる。

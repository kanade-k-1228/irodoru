# 開発者向け情報

[README](../README.md) · [アプリの使い方](Usage.md) · [Docker](Docker.md)

## 準備・起動

stable Rust、Python 3.10以降、uv、GPUI Kitのネイティブ依存ライブラリが必要です。Linuxの依存パッケージは [CIの設定](../.github/workflows/app.yml) を参照してください。実行にはVulkan対応のWayland/X11環境と、`xdg-desktop-portal` およびデスクトップに合ったバックエンドが必要です。Node.jsは使いません。

リポジトリのルートで実行します。

```bash
cargo xtask models export    # animeモデルとcodecをエクスポート
cargo xtask models prepare   # 開発用モデルを配置
cargo xtask dev              # デスクトップアプリを起動
```

既存のモデルがあれば最初の2行を `cargo xtask models prepare --source /path/to/models` に置き換えます。`IRODORU_MODELS` で `anime/` と `codec/` を含むディレクトリを指定することもできます。ローカルの配布マニフェストは仮の値なので、開発時はモデルの準備が必要です。

## 構成

| 場所 | 役割 |
| --- | --- |
| `pkgs/app` | Rust・GPUI製のデスクトップアプリ |
| `pkgs/engine` | 共通の音声合成処理。ONNX Runtime、オプションでOpenVINO |
| `pkgs/engine/script` | Pythonによるモデルのエクスポート・検証 |
| `pkgs/server` | Axum製HTTP API。アプリと同じエンジンを使用 |
| `xtask` | 開発・検証・配布コマンド |
| `scripts`、`.github/workflows` | 配布の補助処理とCI |

モデルの準備・検証・ZIP作成・GHCR公開はRustの `xtask` にまとめています。開発時は `cargo xtask models ...`、CIではreleaseビルドしたxtaskを使います。ONNXエクスポートのみ、PyTorchを使うPythonスクリプトを呼び出します。

アプリ内は `desktop` が起動・ウィンドウ管理、`workspace` が編集状態とUI、`project` がプロジェクトの保存・寿命管理、`library` がファイル操作、`synthesis` がモデル管理と生成を担当します。共有データ型は `domain`、音声入出力は `audio`、設定は `preferences` に置きます。

UI操作は `Command → Service → Worker`、結果は `Event → State → UI` の順に流れます。状態管理はGUIから独立させ、ファイル操作と推論はバックグラウンドで実行します。プロジェクトはウィンドウごとに持ち、推論エンジンとテーマ設定は共有します。

`.iro` は `project.yaml`、`characters/`、`voices/` を含むZIPです。素材は相対パスで参照し、自動保存は新しいZIPの書き込みに成功してから置き換えます。プロジェクトの切り替え・終了時には保存完了を待ちます。

## 検証

```bash
cargo xtask verify          # Pythonテスト、fmt、Clippy、Rustテスト、OpenAPI差分
cargo xtask verify-export   # モデル変換処理のテスト
cargo xtask generate        # OpenAPIを更新
# GUI部分も変更した場合
cargo clippy --locked -p irodoru --features desktop --all-targets -- -D warnings
cargo test --locked -p irodoru --features desktop
```

実モデルを使うテストや配布物の検証はCIも参照してください。日本語入力、ドラッグ操作、音声デバイスは各OSでの操作確認が必要です。

## 配布

```bash
cargo install cargo-packager --version 0.11.8 --locked
cargo xtask package
```

Linuxでは `patchelf`、Windowsでは7-Zipも必要です。`--no-bundle` を付けるとインストーラー化せずアプリディレクトリを作ります。アプリの配布物にモデルは含みません。

`cargo xtask models distribute` でモデルZIPと配布マニフェストを作成します。`1.2.3` のようなタグをpushすると、release workflowがappとdocker workflowを並列に呼び出します。appはモデルをGHCRへ公開してマニフェストを組み込んだアプリをビルドし、dockerは独立してDockerイメージをビルド・公開します。初回に公開アクセスの検証で止まったら、GHCRのモデルパッケージをPublicにして再実行してください。公開済みアプリが参照する古いモデルは削除しないでください。

app・docker両方の成功後、release workflowの最後の `release` jobが各OSの配布物・チェックサム・Dockerのpull手順をまとめて添付し、GitHub Releaseを公開します。リリースノートはGitHubの自動生成機能を使い、分類は `.github/release.yml` で設定します。`1.2.3-beta.1` のようなタグはprereleaseになります。アップロード中はdraftにし、失敗時はCIを再実行できます。main・PRの通常ビルドではReleaseを作成しません。

GHCRは `irodoru` と `irodoru-models` の2パッケージ構成です。`irodoru` のバージョンタグにはDockerイメージ、`cache-amd64`・`cache-arm64` タグにはCIのビルドキャッシュを保存します。キャッシュはmainのCIだけが更新します。`irodoru-models` はデスクトップアプリ用のモデルZIPを保存します。旧 `irodoru-ci-cache` は参照しないため、移行後のmain CIで新しいキャッシュが生成されたら削除できます。

配布物の動作確認には `IRODORU_TEST_MODELS=/absolute/path/to/prepared-models bash scripts/smoke-package.sh TARGET` を使います。モデルには `models prepare` で作成した `bundle.json` が必要で、`TARGET` は対象のRustターゲットです。そのほかのコマンドは `cargo xtask help` で確認できます。

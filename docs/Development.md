# 開発環境の立ち上げ

## 準備

Rust（stable）、uv、Python 3.10をインストールしてください。

Linuxでは、[CI設定](../.github/workflows/app.yml)の「Linux desktop dependencies」と同じパッケージが必要です。実行環境にはVulkan、WaylandまたはX11、`xdg-desktop-portal` とデスクトップ環境に対応するバックエンドを用意します。

```bash
git clone https://github.com/kanade-k-1228/irodoru.git
cd irodoru
```

## モデルを準備して起動

リポジトリのルートで実行します。

```bash
cargo xtask models export    # モデルをダウンロードしてONNXへ変換
cargo xtask models prepare   # 開発用の場所へ配置
cargo xtask dev              # アプリを起動
```

初回はモデルのダウンロード・変換とビルドに時間がかかります。2回目以降は `cargo xtask dev` だけで起動できます。

モデルを用意済みの場合は、最初の2つのコマンドを次に置き換えます。`/path/to/models` には `anime/` と `codec/` を含むディレクトリを指定してください。

```bash
cargo xtask models prepare --source /path/to/models
```

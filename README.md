# irodoru

キャラクターの声で台本を読み上げる音声合成アプリです。[Irodori-TTS](https://github.com/Aratako/Irodori-TTS) を使い、モデルの初回ダウンロード後はオフラインで動作します。

## インストール

[Releases](https://github.com/kanade-k-1228/irodoru/releases) から、OSに合ったファイルをダウンロードしてください。

| OS | インストール・起動 |
| --- | --- |
| Windows x64 | ZIPを展開し、`irodoru.exe` を起動 |
| macOS 15以降・Apple Silicon | DMGを開き、アプリを「アプリケーション」にコピー |
| Linux x64 | debをインストール、またはAppImageに実行権限を付けて起動 |

初回起動時はモデル（約3.44 GB）をダウンロードするため、インターネット接続と十分な空き容量が必要です。分割された配布ファイルは、すべて取得して同梱の説明に従って結合してください。

## まず使ってみる

1. 左上の **irodoru** メニューから「新規プロジェクト…」を選び、`.iro` ファイルを作ります。
2. キャラクター一覧の「＋」で名前と声のプロンプトを設定し、保存します。
3. キャラクター名をクリックして台本に行を追加し、セリフを入力して「生成」を押します。
4. 生成した音声を再生し、プレーヤーのダウンロードアイコンからWAVで保存します。

台本や確定したキャラクター設定は自動保存されます。

- [詳しい使い方](docs/Usage.md)
- [Dockerイメージの使い方](docs/Docker.md)
- [開発者向け情報](docs/Dev.md)

## ライセンス

[MIT](LICENSE)。依存ライブラリとモデルには、それぞれのライセンスが適用されます。モデルの[利用制限](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions)も確認してください。

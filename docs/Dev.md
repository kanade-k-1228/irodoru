# 開発ガイド

[概要](../README.md) · [使い方](Usage.md) · [Docker](Docker.md)

## 開発環境

- 安定版のRust、Python 3.10以降、uv、GPUI Kitが必要とするネイティブライブラリをインストールします。Node.jsは不要です。
- Linuxの必要パッケージは[CI設定](../.github/workflows/app.yml)を参照してください。アプリの実行にはVulkan、WaylandまたはX11、`xdg-desktop-portal` とデスクトップ環境に対応するバックエンドが必要です。

リポジトリのルートで実行します。

```bash
cargo xtask models export    # 音声モデルとコーデックをエクスポート
cargo xtask models prepare   # 開発用モデルを配置
cargo xtask dev              # デスクトップアプリを起動
```

開発ビルドでも依存ライブラリを最適化するため、初回のビルドには時間がかかります。

モデルを用意済みなら、最初の2つのコマンドを `cargo xtask models prepare --source /path/to/models` に置き換えられます。`anime/` と `codec/` を含むディレクトリを環境変数 `IRODORU_MODELS` に指定することもできます。リポジトリ内の配布マニフェストは仮の値なので、ローカル開発ではモデルの準備が必要です。

## リポジトリの構成

| 場所 | 役割 |
| --- | --- |
| `pkgs/app` | RustとGPUIによるデスクトップアプリ。[内部構成](../pkgs/app/README.md)にモジュールと状態の流れを記載しています。 |
| `pkgs/engine` | ONNX Runtimeを使う共通の音声合成エンジン。OpenVINOにも任意で対応します。公開型はクレートのルートに集約し、グラフ・コーデック・モデル・キャッシュの内部実装は非公開です。 |
| `pkgs/model-bundle` | アプリとxtaskで共有する配布マニフェスト、必須ファイル、パス・サイズ・ハッシュの検証。ファイルシステム、通信、GUI、推論には依存しません。 |
| `pkgs/engine/script` | Pythonによるモデルのエクスポートと検証。 |
| `pkgs/server` | アプリと同じエンジンを使うAxumのHTTP API。 |
| `xtask` | 開発、検証、パッケージ作成、モデル配布のコマンド。 |
| `scripts`・`.github/workflows` | パッケージ作成の補助スクリプトとCI。 |

ローカルでは `cargo xtask models ...` を使い、CIではリリースビルドしたxtaskを使います。モデルの準備・検証・ZIP作成・GHCRへの公開はRustで行い、ONNXへのエクスポートはPyTorchを使うPythonを呼び出します。

## 検証

```bash
cargo xtask verify          # フォーマット、Clippy、OpenAPIの差分を検証
cargo test --locked -p irodoru  # アプリの回帰テスト
cargo xtask generate        # OpenAPIを再生成
```

アプリには音声のシーク、セリフ詳細の開閉、使用中キャラクターの削除防止などの回帰テストがあります。配布パッケージの検査はCI設定を参照してください。複数ウィンドウ、自動保存、日本語入力、ドラッグ操作、音声デバイスは各OSで手動確認します。

## パッケージの作成とリリース

```bash
cargo install cargo-packager --version 0.11.8 --locked
cargo install cargo-about --version 0.9.3 --locked --features cli
cargo fetch --locked
cargo xtask package
```

- Linuxでは `patchelf`、Windowsでは7-Zipも必要です。
- Windowsパッケージには実行ファイルとライセンス資料を含め、ランタイムDLLは含めません。Windows MLが利用可能ならそれを使い、それ以外は実行ファイルに静的リンクしたCPU用ONNX Runtimeを使います。
- `--no-bundle` を付けるとインストーラーを作らず、アプリのディレクトリだけを生成します。アプリのパッケージにはモデルを同梱しません。
- モデルZIPと配布マニフェストは `cargo xtask models distribute` で作成します。

### ライセンス資料

パッケージ作成時は `cargo-about` で固定済みの依存関係を調べ、対応する全ターゲットとビルド依存関係を含む共通の `licenses/` を作成します。特定のOSには含まれない部品も記載されます。単独で生成する場合は、先に `cargo fetch --locked` を実行してから `cargo xtask licenses --destination /path/to/output` を使います。資料の生成中は通信しません。MPL対象のソースは `.crate` アーカイブ（gzip圧縮したtar）として同梱します。

デスクトップCIはLinuxで一度だけ資料を生成し、各OSのビルドへ渡します。パッケージ側では `cargo xtask package --licenses-dir "$RUNNER_TEMP/licenses"` で再利用します。キャッシュはRustのビルドキャッシュと分け、ランナーの一時ディレクトリに保存します。

キャッシュキーには `Cargo.lock`、ワークスペースのマニフェスト、`about.toml`、ライセンス本文、`xtask/src/licenses.rs` を含めます。リリースのバージョン更新後にキーを計算します。無関係なxtaskコードやワークフローの変更だけでは無効化しません。GitHubのブランチ単位のキャッシュを使い、PRからはmainのキャッシュも利用できます。ローカルで資料を再利用するときも、これらの入力を変更したら生成し直してください。`--licenses-dir` を省略すると毎回生成します。

| 配布形式 | ライセンス資料の場所 |
| --- | --- |
| Windows ZIP | `licenses/` |
| macOSアプリ | `Contents/Resources/licenses/` |
| Linuxパッケージ | `usr/lib/irodoru/licenses/` |
| Docker | `/app/licenses/` |

`THIRD-PARTY-NOTICES.txt` にライセンス本文、`THIRD-PARTY-SOURCES.txt` にバージョンとソースの入手方法を記載します。生成先として置き換えられるのは、空のディレクトリか以前生成した資料だけです。

依存関係を更新したら `about.toml` と生成した資料を確認します。クレートに元の著作権表記がない場合は上流を調査し、`licenses/clarifications.json` にバージョンを限定して補います。ネイティブのONNX RuntimeとDirectMLは別に確認し、`ort-sys` の更新時には `licenses/native/` も見直します。任意のOpenVINO GPUイメージでは、インストールしたwheelのライセンス資料を保持します。

モデルは `licenses/model/revisions.json` に記録した確認済みのリビジョンから取得します。更新時はモデルカードと派生元のライセンスを確認し、資料を更新して再エクスポートしてください。各モデルとコーデックには `LICENSES.txt` と `provenance.json` が必要です。これらを持たない古いモデルは、配布前に再エクスポートします。第三者のライセンス本文は原文を保持します。

### リリースの流れ

1. `1.2.3` などのタグをプッシュします。アプリとDockerのワークフローが並列で動きます。
2. アプリのワークフローはモデルをGHCRへ公開し、その配布マニフェストを使って各OSのアプリを作ります。Dockerのワークフローは独立してイメージを公開します。
3. 両方が成功すると、最後の `release` ジョブが各OSのパッケージを添付したGitHub Releaseを公開します。

Dockerの操作は[Dockerの使い方](Docker.md)を参照してください。ダウンロードしたファイルはGitHubが表示するSHA-256で検証できます。独立したチェックサムファイルは生成しません。

アプリのファイル名は `irodoru_<version>_<os>_<arch>.<extension>` です。例えば `irodoru_0.0.6_windows_x64.zip`、`irodoru_0.0.6_macos_arm64.dmg`、`irodoru_0.0.6_linux_x64.deb`、`irodoru_0.0.6_linux_x64.AppImage` になります。バージョンにはタグのプレリリース接尾辞も含めます。分割ファイルには `.part001`、`.part002` などを付けます。命名処理は `cargo xtask release app TAG TARGET DEST` で実行できます。

リリースノートは `.github/release.yml` に従いGitHubが生成します。`1.2.3-beta.1` のようなタグはプレリリースになります。アップロード中は下書きのまま保持し、失敗した場合はCIを再実行します。通常のmain・PRビルドではリリースしません。

GHCRのパッケージは次のとおりです。

- `irodoru`: バージョンタグのDockerイメージと、`cache-amd64`・`cache-arm64` のCIキャッシュ。キャッシュを更新するのはmainのCIだけです。
- `irodoru-models`: デスクトップ用のモデルZIP。公開アクセスの初回確認で失敗した場合はパッケージを公開に変更し、CIを再実行します。リリース済みアプリが参照するモデルは保持してください。
- `irodoru-ci-cache`: 旧キャッシュ。mainのCIで置き換え先のキャッシュが作成された後は削除できます。

パッケージに必要なライセンスがあり、モデルの重みが混入していないことを検査します。

```bash
bash scripts/check-package.sh TARGET
```

`TARGET` はRustのターゲット名に置き換えてください。この検査ではモデルの読み込みや音声合成は行いません。その他のコマンドは `cargo xtask help` で確認できます。

<a id="windows-ml-runtime"></a>

## Windows MLランタイム

Windows版にはWindows 11 24H2以降が必要です。CPU推論はランタイムの追加インストールなしで動きます。GPU/NPUを使うには、x64の **Windows App SDK Runtime 2.xの2.6.5以降**が必要です。公式ランタイムをインストールするか、PowerShellで確認済みのインストールスクリプトを実行します。CIも同じスクリプトを使います。

```powershell
./scripts/install-windows-ml.ps1
```

スクリプトは固定した公式NuGetパッケージを取得し、SHA-256を確認してから、署名済みのフレームワークと付属MSIXを現在のユーザーにインストールします。これらのバイナリはアプリのZIPには追加しません。

起動時にプロセス全体で1つのORT APIを選びます。共有パッケージを読み込めない場合、またはAPI 27に対応していない場合は、`ort` の事前ビルド済みアーカイブから静的リンクしたCPUランタイムを使います。別途ネイティブビルドは必要ありません。2種類のランタイムのハンドルは混在させません。Windows MLを選んだ場合は、CPUへ切り替えたセッションも同じ共有ランタイムを使います。

Windowsの動的依存関係APIで `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe` をプロセスのパッケージ依存関係に追加します。解決したパッケージの絶対パスからONNX RuntimeとWindows MLカタログを読み込みます。Rustの `ort` は、確認済みのMicrosoft.Windows.AI.MachineLearning 2.4.89のヘッダーに対応するAPI 27を使い、API 31は要求しません。

Windows MLが認定済みの実行プロバイダーと依存関係を準備し、エンジンがそのライブラリパスを自身のORT環境へ登録します。標準ではGPU/NPUを自動選択します。元のONNXグラフと独自のSnake演算子を使い、自動選択ではグラフごとのCPUへの切り替えを許可します。CPUを明示した場合は自動選択しません。GPU/NPUを明示して対応デバイスがない場合やセッションを作れない場合はエラーにしますが、未対応の個別演算がCPUで実行される場合はあります。初回はカタログからプロバイダーをダウンロードすることがあるため、オフラインで使う前に準備してください。

配布予定の実行ファイルを検査します。

```powershell
./scripts/check-windows-runtime.ps1 -Executable target/x86_64-pc-windows-msvc/release/native/irodoru.exe
# 対応GPUがある環境で実行。必要に応じて実行プロバイダーをダウンロードします。
./scripts/check-windows-runtime.ps1 -Executable target/x86_64-pc-windows-msvc/release/native/irodoru.exe -Gpu
```

検査は実行ファイルだけを一時ディレクトリへコピーして起動し、アプリと同じ場所のDLLに依存しないことを確認します。静的リンクしたランタイムが参照する `DirectML.dll` は、WindowsのSystem32にあるものを使います。アプリは使用したフレームワークパッケージか内蔵CPUランタイムを報告します。

`-EmbeddedCpu` を付けると、Windows MLがあっても新しい診断プロセスで内蔵CPUランタイムを使います。CIはWindows MLのインストール前に内蔵側を、インストール後に共有側を検査し、それぞれ小さなAddグラフをCPUで実行します。GPU検査ではCPUへの切り替えを無効にしてAddを実行します。実機のGPU/NPU、音声合成全体、オフラインでの再利用はWindowsで別に確認してください。読み込み検査の成功だけでは、これらの動作を確認したことにはなりません。

参考: [Windows MLの配布](https://learn.microsoft.com/windows/ai/new-windows-ml/distributing-your-app)、[実行プロバイダーの登録](https://learn.microsoft.com/windows/ai/new-windows-ml/register-execution-providers)、[確認済みランタイム](https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime/2.6.5)。

フッターはエンジンの状態だけを表示します。各グラフの実行プロバイダーとCPUへの切り替え状況は、モデル設定の「詳細」で確認します。この情報はセッションの選択先であり、演算ごとの計測ではありません。アクセラレーターを選んだセッションでも、未対応の演算はCPUで動く場合があります。実行時の失敗でCPUへ切り替わると、表示も更新されます。

## macOSのCore ML

macOSビルドは `ort/coreml` を有効にし、Core ML対応の確認済みORT 1.31.0 Apple Siliconアーカイブを使います。Core MLはOSが提供するため、追加ランタイムは不要です。対応OSはmacOS 15以降です。

標準は自動選択です。MLProgram形式とすべての計算ユニットを有効にしてCore MLを登録します。元のONNXグラフとSnake演算子は、未対応部分をCPUで処理して利用します。プロバイダーの登録エラーはエンジンへ伝え、自動選択ならCPUだけのセッションを再構築します。実行時の失敗もWindowsと同じ仕組みでグラフをCPUへ切り替え、プロバイダー表示を更新します。

CPUを明示した場合はCore MLを登録しません。GPU/NPUの指定は、それぞれ `CPUAndGPU`・`CPUAndNeuralEngine` の方針を選びます。特定ハードウェアでの実行を保証するものではなく、デバイス一覧にもその方針を表示します。OpenVINOの明示的な指定も引き続き利用できます。

モデルの詳細には `Core ML（自動選択）` または選択した計算方針を表示します。実際の演算先はCore MLが決めるため、この表示だけではGPUやNeural Engineで動いたことを確認できません。モデル交換時に古い重みを使わないよう、コンパイル済みモデルのキャッシュは有効にしていません。macOSのCIはネイティブコードとパッケージを検査します。音質、待ち時間、実際のアクセラレーター使用状況はMacで確認してください。

参考: [Core ML実行プロバイダー](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)。

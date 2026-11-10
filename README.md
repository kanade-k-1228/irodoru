# irodoru

A text-to-speech app for reading scripts in character voices, powered by [Irodori-TTS](https://github.com/Aratako/Irodori-TTS). It works offline after the required models and execution providers are installed.

## Install

Download the package for your OS from [Releases](https://github.com/kanade-k-1228/irodoru/releases):

- **Windows 11 24H2 or later, x64:** Extract the ZIP and run `irodoru.exe`.
- **macOS 15 or later, Apple Silicon:** Open the DMG and copy the app to Applications.
- **Linux x64:** Install the deb, or make the AppImage executable and run it.

The first launch downloads about 3.44 GB of models. Allow enough disk space and an internet connection. For split packages, download all parts and follow the included assembly instructions.

## System requirements

- **Windows:** Windows 11 24H2 (build 26100) or later, x64. No additional runtime installation is needed for CPU inference: CPU ONNX Runtime is statically linked into the executable. For GPU/NPU acceleration, install **Windows App SDK Runtime 2.6.5 or later in the 2.x family** and compatible drivers. Windows ML manages hardware-specific providers and may download them on first use. If Windows ML is unavailable, the app automatically uses the embedded CPU runtime; accelerator failures in automatic mode also fall back to CPU. No runtime DLLs are bundled. The footer and model settings show the selected providers per graph, including CPU fallback. See [Windows ML setup](docs/Dev.md#windows-ml-runtime).
- **macOS:** macOS 15 or later on Apple Silicon. Uses the built-in Core ML framework for automatic CPU/GPU/Neural Engine selection; no additional runtime installation is needed. Unsupported operations run on CPU, and automatic mode falls back to ONNX Runtime CPU if Core ML session creation or execution fails. The provider display shows Core ML selection, not the hardware used by each operation.
- **Linux:** x64, using the deb or AppImage package.

## Quick start

1. Click the project label at the bottom left to open **File** and create a `.iro` project.
2. Click **+** in the character list, enter a name and voice prompt, and save.
3. Select a character, click **+** at the end of the script, enter dialogue, and generate audio.
4. Play the result or use the player's download icon to export a WAV file.

Script edits and saved character settings are saved automatically.

- [User guide](docs/Usage.md)
- [Docker guide](docs/Docker.md)
- [Development guide](docs/Dev.md)

## License

The application is [MIT](LICENSE). Dependencies, models, and bundled runtimes have their own terms. Release packages include license texts, copyright notices, and MPL source archives and download links in `licenses/`. The separately installed Windows ML runtime and its execution providers are covered by their own Microsoft/vendor terms.

The app uses [Irodori-TTS-v4.1-Anime](https://huggingface.co/phasefield-audio/Irodori-TTS-v4.1-Anime), which inherits the base model's [usage restrictions](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions). Do not impersonate someone without consent or generate deceptive audio. Each model and codec includes `LICENSES.txt`; see the [source and license review](licenses/model/NOTICE.txt).

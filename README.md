# irodoru

A text-to-speech app for reading scripts in character voices, powered by [Irodori-TTS](https://github.com/Aratako/Irodori-TTS). It works offline after the first model download.

## Install

Download the package for your OS from [Releases](https://github.com/kanade-k-1228/irodoru/releases):

- **Windows x64:** Extract the ZIP and run `irodoru.exe`.
- **macOS 15 or later, Apple Silicon:** Open the DMG and copy the app to Applications.
- **Linux x64:** Install the deb, or make the AppImage executable and run it.

The first launch downloads about 3.44 GB of models. Allow enough disk space and an internet connection. For split packages, download all parts and follow the included assembly instructions.

## Quick start

1. Open the **irodoru** menu at the top left and create a `.iro` project.
2. Click **+** in the character list, enter a name and voice prompt, and save.
3. Click the character name to add a script line, enter dialogue, and generate audio.
4. Play the result or use the player's download icon to export a WAV file.

Script edits and saved character settings are saved automatically.

- [User guide](docs/Usage.md)
- [Docker guide](docs/Docker.md)
- [Development guide](docs/Dev.md)

## License

[MIT](LICENSE). Dependencies and models have their own licenses. See the model's [usage restrictions](https://huggingface.co/Aratako/Irodori-TTS-v4.1-Small#ethical-restrictions).

# User guide

[README](../README.md) · [Docker](Docker.md) · [Development](Dev.md)

![Script, characters, dialogue editor, and audio player](imgs/main.png)

## Projects

- Click the project path (or the red no-project label) at the left of the footer to open the **File** modal and create, open, or save a project, open a new window, or close the current window. Close it with Escape, the close button, or a click on the backdrop.
- A `.iro` file contains the script, characters, reference audio, icons, and generated audio. Only `.iro` projects are supported.
- Choose **キャラクターを引き継いで新規作成…** in the File modal to create an empty script with all current characters, their icons, and the audio they reference. Shared recordings are copied only once; the source project is preserved.
- Edits are saved automatically. If saving fails, use the File modal's save action to retry.
- Before switching projects, save or cancel character edits and wait for generation to finish.
- Opening a `.iro` file from your OS creates a separate window. A project can be open in only one window at a time.
- On Windows, associate `.iro` files with `irodoru.exe` through **Open with**.

![Project menu for creating, opening, and saving projects](imgs/menu.png)

## Characters

1. Click **+** at the bottom left and enter a name and voice prompt, such as “a calm, clear voice.”
2. Enter sample text and generate a preview.
3. Click the save icon at the top right of the editor. Use **×** to cancel or the pencil icon to edit again.

![Character editor with name, voice prompt, and sample text](imgs/character.png)

Use the duplicate button next to the pencil icon to copy the selected character. Its settings and icon are preserved, and the name receives an unused suffix such as ` - 1`.

Characters belong to their project. Optional assets:

- **Reference audio:** WAV, MP3, FLAC, Ogg Vorbis, or AAC/M4A; 1–60 seconds. Select multiple files when importing; imports are converted to 48 kHz mono WAV. Toggle recordings in the reference list to select several, including previously generated takes. All selected recordings are used together, sharing the model’s reference duration limit. A recording can be shared by several characters.
- **Icon:** PNG, JPEG, WebP, GIF, BMP, TIFF, or SVG; up to 10 MB. Images are cropped to 128 × 128 pixels.

Use **キャラクターファイルを書き出す** in the character editor to save its current settings, icon, and selected reference audio as a portable `.irochar` file. Use **キャラクターファイルを読み込む…** in the File modal to add it to another project. These bundles contain no script and no unrelated takes.

## Edit a script

- The **+** button at the end of the script is always visible and adds an empty line at the end.
- Click a character name to select it, then use the **+** at the end of the script to add a line for that character. Click a script line to edit its dialogue directly in that row.
- Hover over a gap before, between, or after script lines to reveal an add-only **+** button.
- Use the icon buttons inside the right edge of a dialogue field to add above with **＋**, duplicate that row immediately below, or delete it. Each button acts on its own row. Duplicates retain the dialogue, character, and generation settings without sharing generated audio. The last line cannot be deleted.
- Drag a row’s character icon to move the row. A horizontal line marks the destination; dragging near the top or bottom scrolls the script.
- The play button inside the left edge of each dialogue field plays or pauses that row’s generated audio. It is disabled until audio exists. Only the right-side action buttons are separated from the dialogue by a vertical line.
- Press `Enter` to add the next line or `Shift+Enter` for a line break. During Japanese input, `Enter` confirms the IME composition.

## Generate and export audio

- Select a line and generate audio. Speed ranges from **0.5× to 2.0×**; advanced settings include the seed and step count. Start with the defaults.
- Select a dialogue bubble to show a split editor below the text: the character name and take checklist on the left, and generation parameters and buttons on the right. Check a take to use it for that dialogue; the choice is saved with the project. Use the take player inside the bubble to play, pause, or seek. Click its download icon to export WAV.
- Successful generation adds a new selected take and retains earlier takes for comparison. Audio remains in the voice library when its script row is removed, so it can still be used as a character reference. A take currently referenced by a character cannot be deleted until that reference is removed.
- If the first model download fails, retry it. Prepared models are reused for offline generation.

## Settings and shortcuts

Click the settings icon at the far right of the footer to open **Settings**, switch between light and dark mode, and view application information.

![Preferences with color theme settings](imgs/preferences.png)

Use `⌘` instead of `Ctrl` on macOS.

On macOS, the app uses the native menu bar and window title bar, without the custom app header. Windows and Linux retain the custom header. Window position, size, and maximized/fullscreen state are remembered when closing a window. On the next launch, the saved placement is restored if it fits a connected display; otherwise it opens centered at 960×700. Additional windows use GPUI’s cascading placement.

- New window: `Ctrl+Shift+N`
- New project: `Ctrl+N`
- Open project: `Ctrl+O`
- Save project: `Ctrl+S`
- Preferences: `Ctrl+,`
- Close window: `Ctrl+W`

The footer provides three separate modals: **File**, **Model**, and **Settings**.

## Model settings

The model icon at the right of the footer shows preparation and download progress, readiness, or an error. Click it to open model settings, inspect the error details, or retry preparation.

To use local models, choose a folder or enter its path and click **Apply**. The folder must contain both `anime/manifest.json` and `codec/codec.json`. The selection is saved for future launches and shared across windows. **Restore default** returns to automatic model resolution (including development overrides). Model folders can be changed when preparation and generation are idle.

Each new take stores its resolved generation conditions inside the WAV file’s metadata, without a separate YAML file, including the seed, steps, speed, CFG, model, text, caption, and reference identity. Selecting a take restores its dialogue, additional prompt, and editable generation parameters. Exported WAVs keep these conditions when imported again.

Use the export button immediately left of the footer gear to export one WAV per dialogue. Choose a naming rule (number, number + character, number + dialogue, or number + character + dialogue), then select an output folder. Names use the selected take’s recorded character and dialogue; unsupported filename characters are replaced and long names are shortened. A new `irodoru-export` folder (with a numeric suffix if needed) contains `001.wav`, `002.wav`, and so on in script order, using each row’s selected take. If any row has no selected audio, export reports its row number before writing.

Only the current project format (version 2 with single-file WAV voices) is supported. Older project formats are not migrated.

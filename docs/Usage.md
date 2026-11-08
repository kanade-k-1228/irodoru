# User guide

[README](../README.md) · [Docker](Docker.md) · [Development](Dev.md)

![Script, characters, dialogue editor, and audio player](imgs/main.png)

## Projects

- Use the **irodoru** menu at the top left to create, open, or save a project.
- A `.iro` file contains the script, characters, reference audio, icons, and generated audio. Older `.zip` projects also work.
- Edits are saved automatically. If saving fails, use the menu's save action to retry.
- Before switching projects, save or cancel character edits and wait for generation to finish.
- Opening a `.iro` file from your OS creates a separate window. A project can be open in only one window at a time.
- On Windows, associate `.iro` files with `irodoru.exe` through **Open with**.

![Project menu for creating, opening, and saving projects](imgs/menu.png)

## Characters

1. Click **+** at the bottom left and enter a name and voice prompt, such as “a calm, clear voice.”
2. Enter sample text and generate a preview.
3. Click the save icon at the top right of the editor. Use **×** to cancel or the pencil icon to edit again.

![Character editor with name, voice prompt, and sample text](imgs/character.png)

Characters belong to their project. Optional assets:

- **Reference audio:** WAV, MP3, FLAC, Ogg Vorbis, or AAC/M4A; 1–60 seconds. Imports are converted to 48 kHz mono WAV.
- **Icon:** PNG, JPEG, WebP, GIF, BMP, TIFF, or SVG; up to 10 MB. Images are cropped to 128 × 128 pixels.

## Edit a script

- Click a character name to append a line for that character. Select a line and edit its dialogue in the lower center field.
- Click **+** between lines or at the end to add an empty line. Drag the left handle to reorder it.
- Click the selected line's trash icon to delete it and its generated audio. The last line cannot be deleted.
- Press `Enter` to add the next line or `Shift+Enter` for a line break. During Japanese input, `Enter` confirms the IME composition.

## Generate and export audio

- Select a line and generate audio. Speed ranges from **0.5× to 2.0×**; advanced settings include the seed and step count. Start with the defaults.
- Use the lower-right player to play, pause, or seek. Click its download icon to export WAV.
- Regeneration replaces existing audio only after it succeeds.
- If the first model download fails, retry it. Prepared models are reused for offline generation.

## Settings and shortcuts

Choose the color theme or open preferences from the top-left menu to switch between light and dark mode.

![Preferences with color theme settings](imgs/preferences.png)

Use `⌘` instead of `Ctrl` on macOS.

- New window: `Ctrl+Shift+N`
- New project: `Ctrl+N`
- Open project: `Ctrl+O`
- Save project: `Ctrl+S`
- Preferences: `Ctrl+,`
- Close window: `Ctrl+W`

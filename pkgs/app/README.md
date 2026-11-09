# Desktop application architecture

The app is one Rust crate. `main.rs` calls `desktop::run`; the `desktop` feature adds GPUI, native dialogs, and playback. Core editing, state, storage, and synthesis can be tested without a display.

## Modules

- `desktop`: Startup, shared inference construction, windows, menus, and platform quit handling.
- `domain`: Shared serializable values, IDs, and errors; no GUI or filesystem access.
- `workspace`: One project window's state, commands, background worker, and UI.
- `project`: Project sessions, ZIP archives, autosave, ownership, and file-open handling.
- `library`: Filesystem storage for project YAML, characters, audio, and assets.
- `synthesis`: Shared inference, generation, and verified model downloads and caching.
- `audio`: Audio import, decoding, resampling, WAV validation, and playback.
- `icons.rs`: Image import and conversion for character icons.
- `preferences`: OS-local YAML settings, an ordered writer, and theme settings. Loading through confy preserves existing settings; writes use atomic file replacement.
- `files.rs`: Private atomic file replacement helper.

## Workspace and UI

- `editor.rs`: Script operations and generation input validation.
- `state.rs` and `state/`: Headless state, editing transitions, event handling, and tests.
- `protocol.rs`: Owned `Command`, `Event`, and `Snapshot` values.
- `service.rs`: UI command handle, notification drain, and stop requests.
- `worker.rs`: Ordered background work and project/engine coordination.
- `view/`: GPUI entities, bindings, dialogs, editing adapters, layout, and playback controls.

Each window owns one `Entity<Studio>` and its `WorkspaceState`. Inputs retain their entities, subscriptions, and focus handles. State owns row and character edits, settings targets, and generation command construction. It consumes worker events and returns `Effect` values without importing GPUI, rodio, rfd, or a repository. Views retain input entities, IME, focus, and scrolling behavior while forwarding edits to state.

```text
GPUI listener → state/editor → Command → Service → Worker → project/library/synthesis
                                    ← Event ←───────────┘
                   state.apply(Event) → Effect → GPUI/device adapter
```

- Build named, stateless view sections in the existing `Studio` context. Keep list iteration separate from item rendering, with script rows and insertion gaps as siblings.
- View modules follow the screen: `layout` composes `script`, `character_panel`, and `footer`; `script/row` owns row actions, the inline editor, and takes; `character_panel` owns the list and editor. Shared controls remain at the view root.
- Keep containers needed for layout, scrolling, or hit testing. Put editing adapters in `editing.rs` and preserve input entities, element IDs, and focus handles.
- Run storage and inference on workers with owned command data. Only the UI thread drains notifications and updates GPUI entities; its weak-entity timer stops when the window closes.
- Use `Command::is_passive` to keep editing available without emitting `Done`. The view and worker share this policy; callers do not supply a separate busy flag.
- The worker resolves characters and reference audio into owned `GenerationInput` values. Generation prepares the complete batch before inference and returns a take plus a plain WAV for the worker to save. The repository alone embeds the app metadata in the WAV. It has no dependency on project or library storage. Takes execute in order, stopping after the current take finishes; failures preserve prior audio.
- `AudioSource` identifies a generated take, a temporary preview, or a saved character preview. Playback and export both read it through `Session::audio`.

## Projects and shared services

- `project::Project` owns the session, archive reservation, and saver. The workspace worker uses `read`, `change`, `open`, and `flush`; it does not manage locks or notify the saver directly.
- Project entry points accept only `.iro` (case-insensitive), and reject other extensions before switching or flushing the current project. A `.iro` project is a ZIP containing `project.yaml`, `characters/`, `voices/`, and `scripts/`, with relative asset paths. Preserve schema compatibility, IDs, atomic writes, autosave timing, and recovery behavior.
- Autosave uses immutable snapshots and one debounced writer. Replace the archive only after the new ZIP is written successfully. Flush pending saves before switching or dropping a project, and report explicit flush failures before closing its window.
- Native file-open events and forwarded launches create independent windows. A per-user instance lock and bounded atomic request inbox preserve launches until the owning process can handle them. A project path can belong to only one window.
- New-window requests are GPUI events handled by `desktop`. Project state is local to each window; theme settings and inference are shared.
- `desktop` constructs one `synthesis::Inference` and passes its handle to each window and workspace service. Inference owns the engine lifecycle, progress, retry, and synthesis queue. Model resolution and settings persistence are callbacks supplied at startup. Local model ZIPs are validated and extracted into app-owned storage on the inference worker. The worker drops the old `Engine`, loads its replacement, and only then saves the installed directory and ZIP display name. Failed imports retain the previous saved source and remove staged files; retry reloads that source. The original ZIP is no longer needed after import. No engine instance is shared behind a lock. ONNX Runtime’s process-wide environment survives replacements; model sessions are rebuilt, and session construction is serialized for custom-op registration.
- Model sources are resolved in order: saved selection, `IRODORU_MODELS`, development resources, then verified cache/download. The app and xtask share the schema 2 manifest and validation in `irodoru-model-bundle`; transport and cache installation stay in the app. Release builds download a GHCR ZIP only when needed; models remain outside the app package.
- `project` and `library` use domain values and audio validation without depending on workspace views or the inference engine. The workspace worker publishes completed takes through the repository.

## Application icons

Keep assets in `icons/` in sync when changing the logo.

- **Windows:** `build.rs` and `windows/app.rc` embed `icon.ico` as resource ID 1 for GPUI window and taskbar icons, including portable ZIP builds.
- **Linux:** X11 windows embed `128x128.png`; Wayland uses the `irodoru` app ID to match the packaged desktop entry.
- **Packages:** `packager.json` supplies PNG/ICNS icons for Linux packages and the macOS app bundle.

## Extend and verify

- Put contracts in `domain` only when they cross module boundaries. Keep workspace-specific state and commands in `workspace`.
- Add input rules to `editor` or state transitions, then connect them through a small view adapter.
- Add I/O commands to `protocol` and implement them in the worker/repository. Return events instead of accessing GPUI contexts.
- Keep ZIP extraction, ownership, and playback implementations private. Use module exports instead of importing private sibling code or adding a general-purpose `utils` module.
- Add a separate crate only for independent reuse, ownership, or build needs. Keep any future database adapter behind the repository boundary.

```bash
cargo xtask verify
cargo clippy --locked -p irodoru --features desktop --all-targets -- -D warnings
cargo test --locked -p irodoru --features desktop
cargo xtask dev
```

- State tests cover save failures, stale notifications, selection, settings scope, character drafts, generation completion, and close guards.
- Storage and worker tests cover `.iro` round trips, unsupported extension rejection, atomic saves and retries, queue cancellation, and regeneration failures.
- Native smoke checks cover multiple windows, shortcut focus, and save-on-close. Check IME input, dragging, and audio devices manually on each platform.

Follow the [GPUI Kit coding guides](https://gpui-kit.com/docs/coding-guides/) and [task model](https://gpui-kit.com/docs/task/) for module boundaries, state ownership, and background work.

## Project format v2

- `project.yaml`: version, ordered `scripts` IDs, common generation preferences, and active character ID.
- `scripts/<id>.yaml`: dialogue, `character_id`, additional prompt, optional per-row settings, and selected `voice_id`.
- `characters/<id>/character.yaml`: character settings and a `voice_ids` array. The optional icon is stored next to the YAML.
- `voices/<id>.wav`: a shared voice table with no metadata sidecars. The standard RIFF `LIST/INFO/ICMT` comment contains JSON under `irodoru_voice`, including the ID and generation conditions. `kind: imported` stores the original recording name; `kind: generated` stores the take's generation snapshot. Resolved parameters live only in `conditions`; text, model, seed, and steps are not duplicated, and default export filenames are derived from those conditions. `script_id`, character information, and input voice IDs in a generated snapshot record its origin; they do not own the audio's lifetime and may refer to a source project after transfer.

Deleting a script does not delete its voices. Explicit audio deletion is rejected while a character references it. Generation reads every selected reference, apportions the model's maximum reference duration across clips, and combines normalized samples before speaker encoding. Single-reference generation retains its previous behavior.

Only the current archive layout is accepted; there is no migration or legacy field-name handling. Saved character previews use a single `preview.wav`. Each editor write stages the complete scripts table and restores it if publishing `project.yaml` fails. Snapshots capture the project under the existing session lock.

Character bundles (`.irochar`) use the same ZIP schema with one character, its icon and referenced voices, and an empty script. Import assigns fresh IDs to avoid replacing existing records. Creating a project from characters preserves shared voice IDs within the new project, copies only referenced voices, and reserves the destination independently so failure leaves the source session open.

WAV exports retain the embedded voice record; importing them preserves generation conditions and assigns a new voice ID. Metadata reads seek past PCM chunks, and metadata updates preserve the original PCM and unrelated RIFF tags.

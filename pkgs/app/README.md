# Desktop application architecture

The app is one Rust crate. `main.rs` calls `desktop::run`; the `desktop` feature adds GPUI, native dialogs, and playback. Core editing, state, storage, and synthesis can be tested without a display.

## Modules

- `desktop`: Startup, windows, menus, and platform quit handling.
- `domain`: Shared serializable values, IDs, and errors; no GUI or filesystem access.
- `workspace`: One project window's state, commands, background worker, and UI.
- `project`: Project sessions, ZIP archives, autosave, ownership, and file-open handling.
- `library`: Filesystem storage for project YAML, characters, audio, and assets.
- `synthesis`: Shared inference, generation, and verified model downloads and caching.
- `audio`: Import, decoding, resampling, WAV validation, and playback.
- `preferences`: OS-local YAML settings through confy, an ordered writer, and theme settings.
- `files.rs`: Private atomic file replacement helper.

## Workspace and UI

- `editor.rs`: Script operations and generation input validation.
- `state.rs` and `state/`: Headless state, editing transitions, event handling, and tests.
- `protocol.rs`: Owned `Command`, `Event`, and `Snapshot` values.
- `service.rs`: UI command handle, notification drain, and stop requests.
- `worker.rs`: Ordered background work and project/engine coordination.
- `view/`: GPUI entities, bindings, dialogs, editing adapters, layout, and playback controls.

Each window owns one `Entity<Studio>` and its `WorkspaceState`. Inputs retain their entities, subscriptions, and focus handles. State consumes worker events and returns `Effect` values without importing GPUI, rodio, rfd, or a repository.

```text
GPUI listener → state/editor → Command → Service → Worker → project/library/synthesis
                                    ← Event ←───────────┘
                   state.apply(Event) → Effect → GPUI/device adapter
```

- Build named, stateless view sections in the existing `Studio` context. Keep list iteration separate from item rendering, with script rows and insertion controls as siblings.
- Keep containers needed for layout, scrolling, or hit testing. Put editing adapters in `editing.rs` and preserve input entities, element IDs, and focus handles.
- Run storage and inference on workers with owned command data. Only the UI thread drains notifications and updates GPUI entities; its weak-entity timer stops when the window closes.
- Use `Command::is_passive` to keep editing available without emitting `Done`. The view and worker share this policy; callers do not supply a separate busy flag.
- Generate prepared takes in order. The ordered worker needs no batch IDs or index lookup. Failed generation preserves prior audio.

## Projects and shared services

- `project::Project` owns the session, archive reservation, and saver. The workspace worker uses `read`, `change`, `open`, and `flush`; it does not manage locks or notify the saver directly.
- A `.iro` project is a ZIP containing `project.yaml`, `characters/`, and `voices/`, with relative asset paths. Preserve schema compatibility, IDs, atomic writes, autosave timing, and recovery behavior.
- Autosave uses immutable snapshots and one debounced writer. Replace the archive only after the new ZIP is written successfully. Flush pending saves before switching or dropping a project, and report explicit flush failures before closing its window.
- Native file-open events and forwarded launches create independent windows. A per-user instance lock and bounded atomic request inbox preserve launches until the owning process can handle them. A project path can belong to only one window.
- New-window requests are GPUI events handled by `desktop`. Project state is local to each window; theme settings and inference are shared.
- `synthesis::Inference` owns shared model initialization. All windows share one inference worker, including model preparation progress and retry. Release builds verify a revisioned cache and download a GHCR ZIP when needed; models remain outside the app package.
- `project` and `library` use domain values and audio validation without depending on workspace views or the inference engine. The generation coordinator publishes completed takes through the repository.

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
- Storage and worker tests cover archive compatibility, atomic saves and retries, queue cancellation, and regeneration failures.
- Native smoke checks cover multiple windows, shortcut focus, and save-on-close. Check IME input, dragging, and audio devices manually on each platform.

Follow the [GPUI Kit coding guides](https://gpui-kit.com/docs/coding-guides/) and [task model](https://gpui-kit.com/docs/task/) for module boundaries, state ownership, and background work.

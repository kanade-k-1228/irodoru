# Desktop application architecture

The application is one Rust crate with capability modules. `main.rs` delegates to
`desktop::run`; the `desktop` feature adds GPUI, native dialogs and playback. Core
editing, state transitions, storage and synthesis can be tested without a display.

```text
src/
├── desktop/                  # Process/window composition, menus, platform quit guard
├── domain/                   # Serializable values, IDs and errors; no GUI or filesystem
├── workspace/                # One project window's application feature
│   ├── editor.rs             # Script operations and generation input validation
│   ├── state.rs, state/      # Headless state, editing transitions, event reducer/tests
│   ├── protocol.rs           # Owned Command/Event/Snapshot values
│   ├── service.rs            # UI-facing command handle and notification drain
│   ├── worker.rs             # Ordered background execution, project/engine coordination
│   └── view/                 # GPUI entity, bindings, dialogs and presentation
│       ├── mod.rs            # Retained input/focus/playback handles and window events
│       ├── bindings.rs       # Input subscriptions and state → input synchronization
│       ├── controller.rs    # Commands and state effects → native side effects
│       ├── editing.rs       # Editing action adapters
│       ├── dialogs.rs       # Async native file/confirmation dialogs
│       ├── layout.rs        # Window composition and keyboard actions
│       ├── header.rs        # App menu trigger, window drag area and window controls
│       ├── app_menu.rs      # Menu items, keyboard navigation and theme control
│       ├── app_dialog.rs    # Preferences and About dialog composition
│       ├── script.rs, line_editor.rs, characters.rs, character_view.rs
│       ├── generation_controls.rs, player.rs, playback.rs
│       └── actions.rs, appearance.rs, components.rs
├── project/                  # Session lifetime and portable project persistence
│   ├── active.rs             # Session access, ownership, change tracking and flush lifecycle
│   ├── session.rs            # Active working directories and revision/status
│   ├── archive.rs            # Immutable save snapshots, safe ZIP extraction/replacement
│   ├── autosave.rs           # Debounce, single writer, flush and retry reporting
│   ├── ownership.rs          # In-process multi-window path reservation
│   └── incoming.rs           # Native file-open parsing and per-user process handoff
├── library/                  # Filesystem repository: project YAML, characters, WAV/assets
├── synthesis/                # Shared engine, generation, verified model download/cache
├── audio/                    # Decode/resample/import, WAV validation, native playback
├── preferences/              # OS-local YAML settings via confy, ordered writer and theme
└── files.rs                  # Private atomic file replacement primitive
```

## Dependencies and ownership

`desktop` composes the application. `workspace::view` owns one `Entity<Studio>` per
window and its plain `WorkspaceState`. Inputs retain their own GPUI entities and
subscriptions; stateless presentation is built as elements. The state module does
not import GPUI, rodio, rfd or a repository. It can consume worker events and return
`Effect` values without opening a window or touching disk.

View entry points compose named sections rather than embedding entire forms or
list-item trees. Each section builds stateless GPUI elements in the same `Studio`
context; extracting presentation does not add entities or duplicate state. Keep
list iteration separate from item rendering, and keep script rows and insertion
controls as siblings. Retain containers needed for layout, scrolling or hit
testing, and remove wrappers that serve none of those purposes. Editing actions
belong in `editing.rs`; presentation helpers keep the existing input entities,
element IDs and focus handles.

A typical action follows this path:

```text
GPUI listener → state/editor → Command → Service → Worker → project/library/synthesis
                                    ← Event ←───────────┘
                   state.apply(Event) → Effect → GPUI/device adapter
```

Storage and inference execute on dedicated workers using owned command data.
`project::Project` owns the active session, archive reservation and saver together.
The workspace worker uses `read`, `change`, `open` and `flush`; it does not manage
project locks or notify the saver itself. Switching and dropping a project flush
pending saves before releasing the old archive. Explicit flush failures are still
reported to the user before a window closes.

`Command::is_passive` defines which commands leave editing available and do not emit
`Done`. Both the view and worker use this policy, so call sites do not pass a separate
busy flag. Generation consumes prepared takes in order; batch IDs and index lookup
are unnecessary inside the ordered worker. Shared model initialization belongs to
`synthesis::Inference`, while `Service` owns only command/event transport and stop requests.

The view drains notifications on the UI thread; only that thread updates retained
GPUI entities. Its weak-entity timer stops when the window disappears. New-window
requests are GPUI events observed by `desktop`, so the feature never calls into the
shell. Project state stays local; theme/settings and inference are coordinated across windows.

Native file-open events and forwarded launches open independent project windows. The per-user
instance lock keeps one process responsible for project ownership; a bounded atomic request inbox
retains launches until that process can receive them. Requests never replace an existing editor.
All windows share one inference worker, including model preparation progress and retry. Release
builds verify a revisioned model cache and download one GHCR ZIP when needed; model weights are
kept outside the app package.

`project` and `library` use domain values and audio validation, and never depend on
workspace presentation or the synthesis engine. The generation coordinator uses
the repository to publish completed takes; failed generation retains prior audio.
Implementation modules such as ZIP extraction, ownership and playback remain
private behind their capability exports. Avoid importing a sibling capability's
private implementation or adding a general-purpose `utils` module.

Persistence is **ZIP/YAML and asset files, not a database**. Archive paths, schema,
IDs, atomic writes, autosave delays, flush-before-close/switch and recovery behavior
are unchanged. A future database adapter belongs behind the repository boundary;
it should not make the view aware of database connections or queries.

## Extending and verifying

Application icons live in `icons/`. Windows builds embed `icon.ico` in the executable
through `build.rs` and `windows/app.rc`, using resource ID 1 for GPUI's window and
taskbar icon. This also covers the portable ZIP distribution. Linux windows embed
`128x128.png` for X11 and use the `irodoru` app ID to match the packaged desktop
entry on Wayland. `packager.json` supplies the PNG/ICNS icons for Linux packages and
the macOS app bundle. Keep these assets in sync when changing the application logo.

Add data contracts to `domain` only when they cross capabilities. Keep feature-only
state and commands in `workspace`. Add input rules to `editor` or state transitions,
then a small view adapter. Add I/O commands to `protocol` and implement them in the
worker/repository, returning events instead of accessing a GPUI context there.
Keep small capabilities as modules; introduce a separate crate only when there is
an independent reuse, ownership or build boundary.

```bash
cargo xtask verify
cargo clippy --locked -p irodoru --features desktop --all-targets -- -D warnings
cargo test --locked -p irodoru --features desktop
cargo xtask dev
```

State regression tests cover save failures, stale status notifications, selection,
settings scope, character drafts, generation completion and close guards. Storage
and worker tests cover archive compatibility, atomic save/retry, queue cancellation
and regeneration failures. Native smoke checks cover multiple windows, shortcut
focus and save-on-close; IME, dragging and audio devices still require platform UI
checks.

The boundaries follow the [GPUI Kit coding guides](https://gpui-kit.com/docs/coding-guides/)
and [task model](https://gpui-kit.com/docs/task/): capabilities first, narrow state
ownership, a composition-only shell and background work outside UI contexts.

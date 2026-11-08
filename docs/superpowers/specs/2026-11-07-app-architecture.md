# Application architecture refactor

Requested in chat: review the whole app, reduce coupling, and make view, state and persistence responsibilities discoverable through directories, following GPUI/Rust GUI practices. Preserve the approved UI, ZIP/YAML schema, generation semantics and all three desktop platforms.

## Approach

Use capability modules in the existing app crate. Splitting only by global models/views/controllers would scatter capabilities; separate crates for every feature would add build and API overhead without independent owners. A small `domain` module contains only shared serializable contracts and IDs, not application services. Public capability facades hide implementation modules.

- `workspace`: editor rules, window-local state transitions, command/event protocol, background service and GPUI presentation. Presentation owns input entities, focus, subscriptions and dialogs. Plain state owns selection, draft, job and error transitions and can be tested without GPUI.
- `project`: session ownership, ZIP encoding/validation, immutable save snapshots, autosave and multi-window path reservations.
- `library`: filesystem repository for project documents, characters, voices and takes. Domain types have no filesystem dependency.
- `synthesis`: engine lifecycle, generation preparation, model resolution and bundle extraction.
- `audio`: decoding/resampling, WAV validation and optional desktop playback.
- `preferences`: settings persistence and the application-wide theme owner.
- `desktop`: startup, windows, native termination and packaged smoke entry point. Windows request another window through an event rather than calling the shell.
- `files`: private atomic file-writing primitive shared by persistence owners.

Dependencies: desktop → workspace presentation → workspace state/protocol/service → project/library/synthesis/audio → domain. Project/library use audio validation rather than depending on synthesis. Domain and state have no GPUI or I/O dependencies. Keep existing worker ordering and autosave timings.

## Verification

Continue existing save, generation, media, legacy ZIP and model tests. Add direct tests for extracted state transitions (failed save, close guards, draft/preview lifecycle, stale status and selection reconciliation). Compile with and without desktop, run Clippy/fmt/OpenAPI checks, and repeat the Linux window/edit/save smoke check. Record remaining platform-only checks honestly.

## Sources

- https://gpui-kit.com/docs/coding-guides/ — capability boundaries, narrow state ownership, Entity lifetimes and value-like presentation.
- https://gpui-kit.com/docs/task/ — background owned data and task lifetime.

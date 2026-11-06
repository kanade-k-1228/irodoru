# App Architecture Implementation Plan

> Execute inline using executing-plans; obtain one fresh whole-change review at the end.

**Goal:** Make application responsibilities and dependency direction explicit while retaining behavior.
**Architecture:** Capability facades, shared domain contracts, headless window state and GPUI-owned presentation state.
**Tech Stack:** Rust, GPUI Kit 0.9, existing ZIP/YAML storage and ordered workers.
**Spec:** ../specs/2026-11-07-app-architecture.md

## Constraints and review focus

Preserve appearance, IDs/schema/paths, autosave debounce/flush protection, stop-after-current generation, previous audio on errors, multi-window ownership, settings ordering and platform entry points. Do not add a new database or replace the synthesis engine.

## Tasks

- [x] Separate domain contracts and capability modules; break persistence → generation dependencies.
- [x] Split project/archive/autosave, library repository, and service protocol/worker behind facades.
- [x] Extract GUI-independent WorkspaceState and transition tests; make the GPUI view an adapter to it.
- [x] Split workspace presentation by responsibility; move bootstrapping to desktop and route window requests through events.
- [x] Document boundaries, run regression/native checks and Linux smoke; complete final review.

## Verification record

- `cargo xtask verify`: passed (63 tests, 4 model-dependent tests ignored), formatting, workspace Clippy and OpenAPI checks passed.
- Desktop Clippy with `-D warnings`: passed. Desktop build and desktop-feature tests passed.
- Seven GUI-independent state tests cover draft retention after failure, selection reconciliation, stale status, close guards, generation completion, preview cleanup and editing/settings scope. The editing API test failed before the extracted operations existed and passed afterwards.
- One independent whole-change review: no remaining Critical/Important findings. Fixed the ignore rule excluding the new `library/` source directory; `git check-ignore` changed from excluded to included and staging confirms all repository sources are present.
- Native Linux smoke passed on a reset project: layout/menu/preferences, two independent windows, Save retaining input focus, and Close flushing the ZIP. The harness now waits for process completion (bounded at 30 seconds) instead of requiring exit within 2 seconds during model startup.
- Cross-platform CI will run on the PR; Windows/macOS interactive IME/device checks remain manual.

Implementation decisions: keep capability boundaries in one crate; ZIP/YAML remains the repository implementation. Preserve the current notification timer and worker ordering during this structural refactor.

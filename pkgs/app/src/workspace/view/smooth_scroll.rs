//! Eased mouse-wheel scrolling for a `ScrollHandle`, as browsers do it.
//!
//! GPUI applies a wheel notch to the offset at once. This turns the jump into
//! a target and moves the offset toward it on every frame. Precise deltas
//! (trackpads) already arrive smoothed and pass through untouched.
use gpui_kit::*;
use std::{cell::Cell, rc::Rc, time::Instant};

/// Time for the remaining distance to shrink by a factor of e.
const TIME_CONSTANT: f32 = 0.07;
/// Distance at which the offset snaps to its target.
const REST: Pixels = px(0.5);
/// Larger differences from the offset written last mean something else scrolled.
const TOLERANCE: Pixels = px(1.);

#[derive(Clone, Copy)]
struct Motion {
    target: Pixels,
    shown: Pixels,
    at: Instant,
}

#[derive(Default)]
struct State {
    /// Offset when the current wheel event arrived, before GPUI applied it.
    before: Cell<Pixels>,
    motion: Cell<Option<Motion>>,
    ticking: Cell<bool>,
}

#[derive(Clone, Default)]
pub(super) struct SmoothScroll {
    handle: ScrollHandle,
    state: Rc<State>,
}

impl SmoothScroll {
    pub(super) fn new(handle: ScrollHandle) -> Self {
        Self {
            handle,
            state: Rc::default(),
        }
    }

    pub(super) fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    /// Records the offset before GPUI's own handler moves it. Place it next to
    /// the scrolling element, not inside it, so child indices stay unchanged.
    pub(super) fn probe(&self) -> impl IntoElement + use<> {
        let this = self.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                window.on_mouse_event(move |_: &ScrollEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        this.state.before.set(this.handle.offset().y);
                    }
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// The `on_scroll` listener for the scrolling element. On the same element
    /// it runs after GPUI's handler, so the offset has already jumped.
    pub(super) fn listener(
        &self,
        view: EntityId,
    ) -> impl Fn(&ScrollEvent, &mut Window, &mut App) + 'static {
        let this = self.clone();
        move |event, window, cx| this.wheel(event, view, window, cx)
    }

    fn wheel(&self, event: &ScrollEvent, view: EntityId, window: &mut Window, cx: &mut App) {
        if event.delta.precise() || cx.reduce_motion() {
            self.state.motion.set(None);
            return;
        }
        let jumped = self.handle.offset();
        let max = self.handle.max_offset().y;
        let before = self.state.before.get().clamp(-max, px(0.));
        let delta = jumped.y - before;
        if delta.is_zero() {
            return;
        }
        let base = self
            .state
            .motion
            .get()
            .filter(|motion| (motion.shown - before).abs() <= TOLERANCE)
            .map_or(before, |motion| motion.target);
        let target = (base + delta).clamp(-max, px(0.));
        self.handle.set_offset(point(jumped.x, before));
        if (target - before).abs() < REST {
            self.state.motion.set(None);
            return;
        }
        self.state.motion.set(Some(Motion {
            target,
            shown: before,
            at: Instant::now(),
        }));
        if !self.state.ticking.replace(true) {
            self.schedule(view, window);
        }
    }

    fn schedule(&self, view: EntityId, window: &Window) {
        let this = self.clone();
        window.on_next_frame(move |window, cx| this.tick(view, window, cx));
    }

    fn tick(&self, view: EntityId, window: &mut Window, cx: &mut App) {
        let offset = self.handle.offset();
        // A scrollbar drag, `scroll_to_item` or drag auto-scroll took over.
        let motion = self
            .state
            .motion
            .get()
            .filter(|motion| (offset.y - motion.shown).abs() <= TOLERANCE);
        let Some(motion) = motion else {
            self.state.motion.set(None);
            self.state.ticking.set(false);
            return;
        };
        let now = Instant::now();
        let elapsed = now.duration_since(motion.at).as_secs_f32();
        let target = motion.target.clamp(-self.handle.max_offset().y, px(0.));
        let mut shown = target + (motion.shown - target) * (-elapsed / TIME_CONSTANT).exp();
        let done = (shown - target).abs() < REST;
        if done {
            shown = target;
        }
        self.handle.set_offset(point(offset.x, shown));
        cx.notify(view);
        if done {
            self.state.motion.set(None);
            self.state.ticking.set(false);
        } else {
            self.state.motion.set(Some(Motion {
                target,
                shown,
                at: now,
            }));
            self.schedule(view, window);
        }
    }
}

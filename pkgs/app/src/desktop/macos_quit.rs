//! Route Cocoa's Dock/OS termination requests through the same save guard as window close.
//! GPUI's app-will-quit notification is too late to cancel and waits only 200 ms.
use gpui_kit::*;
use objc2::{
    class, msg_send,
    runtime::{AnyObject, ClassBuilder, Sel},
    sel,
};
use std::cell::{Cell, RefCell};

thread_local! {
    static ALLOW_EXIT: Cell<bool> = const { Cell::new(false) };
    static REQUEST_CLOSE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

pub fn allow_exit() {
    ALLOW_EXIT.set(true);
}

extern "C-unwind" fn should_terminate(_: &AnyObject, _: Sel, _: *mut AnyObject) -> usize {
    if ALLOW_EXIT.get() {
        return 1; // NSTerminateNow: every window has already flushed successfully.
    }
    REQUEST_CLOSE.with_borrow(|callback| {
        if let Some(callback) = callback {
            callback();
        }
    });
    0 // NSTerminateCancel: retain windows on generation, character editing, or save failure.
}

pub fn install(cx: &App) {
    let async_cx = cx.to_async();
    REQUEST_CLOSE.with_borrow_mut(|callback| {
        *callback = Some(Box::new(move || {
            async_cx.update(|cx| {
                for handle in cx.windows() {
                    let _ = handle.update(cx, |_, window, cx| {
                        window.dispatch_action(
                            Box::new(crate::workspace::view::actions::CloseWindow),
                            cx,
                        );
                    });
                }
            });
        }));
    });
    // GPUI 0.3.11's delegate has no applicationShouldTerminate:. Subclass its existing
    // delegate without adding ivars, preserving GPUI's methods and object layout.
    // Cocoa and this installation both run on the application's main thread.
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let delegate: *mut AnyObject = msg_send![app, delegate];
        let delegate = delegate.as_ref().expect("GPUI application delegate");
        let mut subclass = ClassBuilder::new(c"IrodoruApplicationDelegate", delegate.class())
            .expect("register application termination guard");
        subclass.add_method(
            sel!(applicationShouldTerminate:),
            should_terminate as extern "C-unwind" fn(_, Sel, *mut AnyObject) -> usize,
        );
        AnyObject::set_class(delegate, subclass.register());
        // Refresh Cocoa's cached optional delegate selectors after adding the guard.
        let _: () = msg_send![app, setDelegate: delegate];
    }
}

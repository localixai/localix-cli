//! The app's Edit menu (macOS): a WebView copies, cuts, pastes, undoes and selects all only when a
//! menu turns Cmd+C, Cmd+V and the rest into those actions, and an accessory app has no menu of
//! its own. Never shown (an accessory app has no menu bar); its keys reach the key window.

use std::sync::OnceLock;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};

/// Each item: its title, its action (sent to the first responder) and its key; an upper-case key
/// takes Shift too.
fn items() -> [(&'static std::ffi::CStr, Sel, &'static std::ffi::CStr); 6] {
    [
        (c"Undo", sel!(undo:), c"z"),
        (c"Redo", sel!(redo:), c"Z"),
        (c"Cut", sel!(cut:), c"x"),
        (c"Copy", sel!(copy:), c"c"),
        (c"Paste", sel!(paste:), c"v"),
        (c"Select All", sel!(selectAll:), c"a"),
    ]
}

fn text(value: &std::ffi::CStr) -> Retained<AnyObject> {
    // SAFETY: NSString makes a string of a NUL-terminated UTF-8 C string, here a literal.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: value.as_ptr()] }
}

fn menu(title: &std::ffi::CStr) -> Retained<AnyObject> {
    // SAFETY: `alloc` answers an NSMenu to initialize.
    let made: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenu), alloc] };
    // SAFETY: `initWithTitle:` takes an NSString and answers the menu, retained.
    unsafe { msg_send![made, initWithTitle: &*text(title)] }
}

fn item(title: &std::ffi::CStr, action: Option<Sel>, key: &std::ffi::CStr) -> Retained<AnyObject> {
    // SAFETY: `alloc` answers an NSMenuItem to initialize.
    let made: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenuItem), alloc] };
    // SAFETY: `initWithTitle:action:keyEquivalent:` takes two NSStrings and a selector, which may
    // be nil, and answers the item, retained.
    unsafe {
        msg_send![
            made,
            initWithTitle: &*text(title),
            action: action,
            keyEquivalent: &*text(key)
        ]
    }
}

/// Give the app its Edit menu, once. Runs on the main thread, where the event loop is.
pub(super) fn install() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        let edit = menu(c"Edit");
        for (title, action, key) in items() {
            // SAFETY: an NSMenu takes an NSMenuItem.
            let _: () = unsafe { msg_send![&*edit, addItem: &*item(title, Some(action), key)] };
        }
        let holder = item(c"Edit", None, c"");
        // SAFETY: an NSMenuItem takes an NSMenu as its submenu.
        let _: () = unsafe { msg_send![&*holder, setSubmenu: &*edit] };
        let bar = menu(c"");
        // SAFETY: as above, an NSMenu takes an NSMenuItem.
        let _: () = unsafe { msg_send![&*bar, addItem: &*holder] };
        // SAFETY: NSApplication's class method answers the app.
        let app: Retained<AnyObject> =
            unsafe { msg_send![class!(NSApplication), sharedApplication] };
        // SAFETY: the app takes an NSMenu as its main menu, and retains it.
        let _: () = unsafe { msg_send![&*app, setMainMenu: &*bar] };
    });
}

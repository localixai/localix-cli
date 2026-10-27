//! The deck and the mini as panels that never activate the app (macOS, ARCHITECTURE.md A9), in
//! AppKit's own messages. tao builds an NSWindow; here it becomes a non-activating NSPanel at
//! runtime, as tauri-nspanel does it: a registered NSPanel subclass, the object's class swapped,
//! the non-activating style, and (a private selector, asked for first) activation prevented. A
//! panel with that style comes over the Space the person is in, a full-screen app's too, and
//! becomes the key window only when a click needs it (a text field), never making llx the
//! active app: the person's keys stay where they were. Nothing here calls activate.

use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;

use super::capture::CGRect;
use super::placement::Area;
use super::platform::unseen;

/// NSWindowStyleMask.
const NONACTIVATING_PANEL: usize = 1 << 7;
/// NSWindowCollectionBehavior: on every Space, over a full-screen app, out of window cycling.
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
/// NSWindowLevel: floating, above the ordinary windows.
const FLOATING: isize = 3;

/// Which window it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Deck,
    Mini,
}

extern "C" fn yes(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

extern "C" fn no(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::NO
}

/// An NSPanel that may become the key window (a text field in its page needs it) and never the
/// main one.
fn panel_class() -> Option<&'static AnyClass> {
    static CLASS: OnceLock<Option<&'static AnyClass>> = OnceLock::new();
    *CLASS.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxPanel", AnyClass::get(c"NSPanel")?)?;
        // SAFETY: the methods have NSWindow's signatures: no argument, a BOOL result.
        unsafe { class.add_method(sel!(canBecomeKeyWindow), yes as extern "C" fn(_, _) -> Bool) };
        // SAFETY: as above.
        unsafe { class.add_method(sel!(canBecomeMainWindow), no as extern "C" fn(_, _) -> Bool) };
        Some(class.register())
    })
}

/// `window`'s NSWindow, for as long as the borrow lasts.
fn native(window: &Window) -> &AnyObject {
    // SAFETY: tao's NSWindow is alive for as long as its `Window` is, which is borrowed here.
    unsafe { &*window.ns_window().cast::<AnyObject>() }
}

/// Whether `object` takes `selector`: private ones are asked for before they are used.
fn takes(object: &AnyObject, selector: Sel) -> bool {
    // SAFETY: `respondsToSelector:` takes a selector and answers a BOOL.
    let yes: Bool = unsafe { msg_send![object, respondsToSelector: selector] };
    yes.as_bool()
}

/// Make `window` a floating, non-activating panel on every Space, of `role`. Where the class
/// cannot be swapped (the new one needs more room than tao's own window class has) it stays an
/// ordinary window with the same level and spaces: it still never takes the keyboard on its own.
pub(super) fn adopt(window: &Window, role: Role) {
    let object = native(window);
    let swapped =
        panel_class().filter(|class| class.instance_size() <= object.class().instance_size());
    if let Some(class) = swapped {
        // SAFETY: NSPanel's subclass has no instance variable of its own, and NSPanel is no
        // larger than tao's window class (checked just above), whose one extra variable lies
        // past what NSPanel uses and is read only by tao's own overrides, now gone.
        unsafe { AnyObject::set_class(object, class) };
        // SAFETY: a window takes and answers an NSWindowStyleMask (an NSUInteger).
        let mask: usize = unsafe { msg_send![object, styleMask] };
        // SAFETY: as above; the panel style is only added to what the window has.
        let _: () = unsafe { msg_send![object, setStyleMask: mask | NONACTIVATING_PANEL] };
        let prevent = sel!(_setPreventsActivation:);
        if takes(object, prevent) {
            // SAFETY: the window answers this private selector (checked just above); a BOOL.
            let _: () = unsafe { msg_send![object, _setPreventsActivation: Bool::YES] };
        }
        // SAFETY: an NSPanel takes a BOOL.
        let _: () = unsafe { msg_send![object, setFloatingPanel: Bool::YES] };
        // SAFETY: as above: a click that needs no keyboard (a button) never makes it key.
        let _: () = unsafe { msg_send![object, setBecomesKeyOnlyIfNeeded: Bool::YES] };
    }
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setHidesOnDeactivate: Bool::NO] };
    let behavior = match role {
        Role::Deck => CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY,
        Role::Mini => CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | STATIONARY | IGNORES_CYCLE,
    };
    // SAFETY: an NSWindow takes an NSWindowCollectionBehavior (an NSUInteger).
    let _: () = unsafe { msg_send![object, setCollectionBehavior: behavior] };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: FLOATING] };
}

/// On screen, as it looks: opaque to the eye and the mouse. Under `LLX_WINDOWS=unseen` it stays
/// invisible and click-through, as every window of a test run does.
pub(super) fn reveal(window: &Window, rendering: Rendering) {
    if unseen() {
        return conceal(window, rendering);
    }
    let object = native(window);
    // SAFETY: an NSWindow takes a CGFloat.
    let _: () = unsafe { msg_send![object, setAlphaValue: 1.0f64] };
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setIgnoresMouseEvents: Bool::NO] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![object, setHasShadow: Bool::YES] };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: FLOATING] };
    front(object);
}

/// How an invisible window keeps its page drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rendering {
    /// WebKit's occlusion switch is off: the window may sit behind every other one.
    Switched,
    /// It is not: the window floats on screen, clear, so that nothing covers it.
    Floating,
}

/// Out of sight and out of the way, and still drawing, at the size it has: clear, passing every
/// click through, behind the other windows (what a headless window is, and WebKit keeps
/// rendering in it, so a page never reflows or stops).
pub(super) fn conceal(window: &Window, rendering: Rendering) {
    conceal_object(native(window), rendering);
}

/// [`conceal`] for a window found through its WebView.
pub(super) fn conceal_object(object: &AnyObject, rendering: Rendering) {
    // SAFETY: an NSWindow takes a CGFloat.
    let _: () = unsafe { msg_send![object, setAlphaValue: 0.0f64] };
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setIgnoresMouseEvents: Bool::YES] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![object, setHasShadow: Bool::NO] };
    let level: isize = if rendering == Rendering::Switched {
        0
    } else {
        FLOATING
    };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: level] };
    if rendering == Rendering::Switched {
        // SAFETY: `orderBack:` takes a sender, which may be nil, and returns nothing.
        let _: () = unsafe { msg_send![object, orderBack: std::ptr::null::<AnyObject>()] };
    } else {
        front(object);
    }
}

/// Take `window` off the screen altogether (the mini, when the window it stands for is open).
pub(super) fn hide(window: &Window) {
    // SAFETY: `orderOut:` takes a sender, which may be nil, and returns nothing.
    let _: () = unsafe { msg_send![native(window), orderOut: std::ptr::null::<AnyObject>()] };
}

fn front(object: &AnyObject) {
    // SAFETY: `orderFrontRegardless` takes no argument and returns nothing; unlike
    // `makeKeyAndOrderFront:` it neither makes the window key nor activates the app.
    let _: () = unsafe { msg_send![object, orderFrontRegardless] };
}

/// The primary screen's room for windows, below its menu bar and above its Dock, in points from
/// the top left.
pub(super) fn work_area() -> Option<Area> {
    // SAFETY: `screens` is NSScreen's class method, answering an array.
    let screens: Retained<AnyObject> = unsafe { msg_send![class!(NSScreen), screens] };
    // SAFETY: an NSArray answers `count` with an NSUInteger.
    let count: usize = unsafe { msg_send![&*screens, count] };
    if count == 0 {
        return None;
    }
    // SAFETY: the array has an element at 0 (checked just above).
    let primary: Retained<AnyObject> = unsafe { msg_send![&*screens, objectAtIndex: 0usize] };
    // SAFETY: an NSScreen answers `frame` with an NSRect, which `CGRect` lays out.
    let frame: CGRect = unsafe { msg_send![&*primary, frame] };
    // SAFETY: as above, for `visibleFrame`.
    let visible: CGRect = unsafe { msg_send![&*primary, visibleFrame] };
    Some(Area {
        x: visible.origin.x,
        y: frame.size.height - visible.origin.y - visible.size.height,
        width: visible.size.width,
        height: visible.size.height,
    })
}

//! The mini's grip (macOS, ARCHITECTURE.md A9): a native view over its page that takes the
//! person's press itself, as openclaw's and DmNote's frameless windows do. A press that moves is
//! handed, with the press itself, to AppKit's own window drag (smooth, snapping, ended by the
//! release wherever it happens); one that does not is a click, and expands the window. The page
//! never sees either: an image there dragged itself out as a file, and a release outside it was
//! missed. Only the page's expand button, in the corner, is left to the page.

use std::sync::{Mutex, OnceLock};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tao::event_loop::EventLoopProxy;
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;

use super::capture::{CGPoint, CGRect};
use super::host::Cmd;
use super::presence::Event;

/// The corner the page keeps: its expand button, in points from the top right.
const BUTTON: f64 = 40.0;
/// How far a press moves before it is a drag, in points.
const SLOP: f64 = 3.0;
/// NSEventMask: the left button dragged, or let go.
const DRAGGED_OR_UP: u64 = (1 << 6) | (1 << 2);
/// NSEventType: the left button let go.
const LEFT_UP: usize = 2;
/// NSViewAutoresizingMask: its width and height follow the window's.
const SIZABLE: usize = (1 << 1) | (1 << 4);

/// Where a click on the grip goes: the loop, told to expand. One mini per process.
static PROXY: OnceLock<Mutex<Option<EventLoopProxy<Cmd>>>> = OnceLock::new();

extern "C" fn yes_for(_this: &AnyObject, _sel: Sel, _event: *mut AnyObject) -> Bool {
    Bool::YES
}

/// Itself, but over the page's button, which the page answers.
extern "C" fn hit_test(this: &AnyObject, _sel: Sel, point: CGPoint) -> *mut AnyObject {
    // SAFETY: a view answers `superview` with a view or nil.
    let parent: *mut AnyObject = unsafe { msg_send![this, superview] };
    // SAFETY: a view converts a point from another view of its window, or from its window (nil).
    let at: CGPoint = unsafe { msg_send![this, convertPoint: point, fromView: parent] };
    // SAFETY: a view answers `bounds` with an NSRect, which `CGRect` lays out.
    let bounds: CGRect = unsafe { msg_send![this, bounds] };
    let inside =
        at.x >= 0.0 && at.y >= 0.0 && at.x <= bounds.size.width && at.y <= bounds.size.height;
    // Not flipped: y grows upwards, so the top right corner is the largest x and y.
    let corner = at.x > bounds.size.width - BUTTON && at.y > bounds.size.height - BUTTON;
    match inside && !corner {
        true => std::ptr::from_ref(this).cast_mut(),
        false => std::ptr::null_mut(),
    }
}

/// The pointer on the screen, in points.
fn pointer() -> CGPoint {
    // SAFETY: NSEvent's class method answers the pointer's place as an NSPoint.
    unsafe { msg_send![class!(NSEvent), mouseLocation] }
}

/// A press: followed until it moves (a drag, AppKit's) or is let go (a click, an expand).
extern "C" fn mouse_down(this: &AnyObject, _sel: Sel, press: *mut AnyObject) {
    // SAFETY: a view answers `window` with its window, or nil.
    let window: *mut AnyObject = unsafe { msg_send![this, window] };
    // SAFETY: the pointers AppKit passed are live for this call; nil is checked.
    let (Some(window), Some(press)) = (unsafe { window.as_ref() }, unsafe { press.as_ref() })
    else {
        return;
    };
    let start = pointer();
    // SAFETY: NSDate's class method answers a date.
    let never: Retained<AnyObject> = unsafe { msg_send![class!(NSDate), distantFuture] };
    // SAFETY: NSString makes a string of a C string that lives for the program.
    let mode: Retained<AnyObject> = unsafe {
        msg_send![class!(NSString), stringWithUTF8String: c"NSEventTrackingRunLoopMode".as_ptr()]
    };
    // SAFETY: NSApplication's class method answers the app.
    let app: Retained<AnyObject> = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    loop {
        // SAFETY: the app waits for the next event of the mask, in the mode, and takes it
        // off the queue: a press's own tracking loop, as AppKit's controls run one.
        let next: Option<Retained<AnyObject>> = unsafe {
            msg_send![&*app, nextEventMatchingMask: DRAGGED_OR_UP, untilDate: &*never, inMode: &*mode, dequeue: Bool::YES]
        };
        let Some(next) = next else { return };
        // SAFETY: an NSEvent answers its type, an NSUInteger.
        let kind: usize = unsafe { msg_send![&*next, type] };
        if kind == LEFT_UP {
            return expand();
        }
        let now = pointer();
        if (now.x - start.x).hypot(now.y - start.y) >= SLOP {
            // SAFETY: a window drags itself with the press that began it; AppKit follows the
            // pointer until the button is let go.
            let _: () = unsafe { msg_send![window, performWindowDragWithEvent: press] };
            return;
        }
    }
}

fn expand() {
    let proxy = PROXY.get().and_then(|held| crate::lock(held).clone());
    if let Some(proxy) = proxy {
        let _ = proxy.send_event(Cmd::Presence(Event::Expand));
    }
}

fn grip_class() -> Option<&'static AnyClass> {
    static CLASS: OnceLock<Option<&'static AnyClass>> = OnceLock::new();
    *CLASS.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxMiniGrip", AnyClass::get(c"NSView")?)?;
        // SAFETY: NSView's `acceptsFirstMouse:` takes an event and answers a BOOL: the first
        // press on the mini acts, though llx is never the active app.
        unsafe {
            class.add_method(
                sel!(acceptsFirstMouse:),
                yes_for as extern "C" fn(_, _, _) -> Bool,
            );
        };
        // SAFETY: NSView's `hitTest:` takes an NSPoint and answers a view or nil.
        unsafe {
            class.add_method(
                sel!(hitTest:),
                hit_test as extern "C" fn(_, _, _) -> *mut AnyObject,
            );
        };
        // SAFETY: NSResponder's `mouseDown:` takes an event and answers nothing.
        unsafe { class.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(_, _, _)) };
        Some(class.register())
    })
}

/// Lay the grip over the mini's page, its clicks told to `proxy`.
pub(super) fn grip(window: &Window, proxy: &EventLoopProxy<Cmd>) {
    *crate::lock(PROXY.get_or_init(|| Mutex::new(None))) = Some(proxy.clone());
    let Some(class) = grip_class() else { return };
    // SAFETY: tao's NSWindow is alive for as long as its `Window` is, which is borrowed here.
    let native = unsafe { &*window.ns_window().cast::<AnyObject>() };
    // SAFETY: a window answers its content view.
    let content: *mut AnyObject = unsafe { msg_send![native, contentView] };
    // SAFETY: nil is checked; the view lives as long as the window.
    let Some(content) = (unsafe { content.as_ref() }) else {
        return;
    };
    // SAFETY: a view answers `bounds` with an NSRect.
    let bounds: CGRect = unsafe { msg_send![content, bounds] };
    // SAFETY: a registered NSView subclass is allocated...
    let made: Allocated<AnyObject> = unsafe { msg_send![class, alloc] };
    // SAFETY: ...and given its frame.
    let view: Option<Retained<AnyObject>> = unsafe { msg_send![made, initWithFrame: bounds] };
    let Some(view) = view else { return };
    // SAFETY: a view takes an NSAutoresizingMaskOptions (an NSUInteger).
    let _: () = unsafe { msg_send![&*view, setAutoresizingMask: SIZABLE] };
    // SAFETY: added last, it lies over the page; the content view keeps it.
    let _: () = unsafe { msg_send![content, addSubview: &*view] };
}

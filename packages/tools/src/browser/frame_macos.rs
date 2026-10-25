//! The glass on macOS, through AppKit's own messages (no bindings beyond what the WebView links).
//! The material sits in the window's frame view, below everything, so it backs the titlebar and
//! the pages alike; a panel's coat sits just above it.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use tao::platform::macos::{WindowBuilderExtMacOS, WindowExtMacOS};
use tao::window::{Window, WindowBuilder};

use super::super::capture::{CGRect, CGSize};

/// NSAutoresizingMaskOptions: width and height follow the frame view.
const SIZABLE: usize = 2 | 16;
/// NSWindowOrderingMode.
const BELOW: isize = -1;
const ABOVE: isize = 1;

/// A window that shows what is behind it, with its titlebar clear over the material. The strip
/// is the titlebar, as in a browser: the pages fill the window from its top, the strip beside the
/// window's buttons, and the title is hidden (the strip names the tabs).
pub(super) fn clear(builder: WindowBuilder) -> WindowBuilder {
    builder
        .with_transparent(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true)
}

/// Whether the person asked for less transparency (Accessibility, Display).
pub(super) fn reduced() -> bool {
    // SAFETY: `sharedWorkspace` is NSWorkspace's class method, returning the shared instance.
    let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    // SAFETY: an NSWorkspace answers this property with a BOOL.
    let reduced: Bool =
        unsafe { msg_send![&*workspace, accessibilityDisplayShouldReduceTransparency] };
    reduced.as_bool()
}

/// What names our material among the frame view's own (the system puts materials there too).
fn name() -> Retained<AnyObject> {
    // SAFETY: the argument is a NUL-terminated UTF-8 literal, alive for the whole program.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c"llx.glass".as_ptr()] }
}

/// What names the coat over it.
fn coat_name() -> Retained<AnyObject> {
    // SAFETY: the argument is a NUL-terminated UTF-8 literal, alive for the whole program.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c"llx.coat".as_ptr()] }
}

/// The view that holds the titlebar and the content.
fn frame_view(window: &Window) -> Option<Retained<AnyObject>> {
    let window = window.ns_window() as *mut AnyObject;
    // SAFETY: `window` is tao's live NSWindow, borrowed for this call; its content view may be
    // nil, which the `Option` takes.
    let content: Option<Retained<AnyObject>> = unsafe { msg_send![window, contentView] };
    let content = content?;
    // SAFETY: an NSView answers `superview` with a view or nil.
    unsafe { msg_send![&*content, superview] }
}

/// A window's material, and the coat over it. Kept, so a new tint changes these two and never
/// adds another.
pub(super) struct Glass {
    effect: Retained<AnyObject>,
    coat: Retained<AnyObject>,
}

impl Glass {
    /// The material under everything in `window`, and its coat.
    pub(super) fn under(window: &Window) -> Option<Self> {
        let frame = frame_view(window)?;
        // SAFETY: an NSView answers `bounds` with an NSRect, which `CGRect` lays out.
        let bounds: CGRect = unsafe { msg_send![&*frame, bounds] };
        let effect = view(class!(NSVisualEffectView), bounds, &name());
        // SAFETY: an NSVisualEffectView takes an NSInteger blending mode: behind the window,
        // the desktop, blurred.
        let _: () = unsafe { msg_send![&*effect, setBlendingMode: 0isize] };
        // Active, always: these windows are never key, and an inactive material stops sampling
        // and paints flat grey.
        // SAFETY: an NSVisualEffectView takes an NSInteger state.
        let _: () = unsafe { msg_send![&*effect, setState: 1isize] };
        let none: Option<&AnyObject> = None;
        // SAFETY: `frame` and `effect` are live views; a nil sibling puts it below them all.
        let _: () = unsafe {
            msg_send![&*frame, addSubview: &*effect, positioned: BELOW, relativeTo: none]
        };
        let coat = view(class!(NSBox), bounds, &coat_name());
        // A custom box: a fill and nothing else, no border and no title.
        // SAFETY: an NSBox takes an NSBoxType (an NSUInteger); 4 is custom.
        let _: () = unsafe { msg_send![&*coat, setBoxType: 4usize] };
        // SAFETY: an NSBox takes an NSTitlePosition (an NSUInteger); 0 is none.
        let _: () = unsafe { msg_send![&*coat, setTitlePosition: 0usize] };
        // SAFETY: an NSBox takes a CGFloat border width.
        let _: () = unsafe { msg_send![&*coat, setBorderWidth: 0.0f64] };
        // SAFETY: `frame`, `coat` and `effect` are live views, `effect` already a subview.
        let _: () = unsafe {
            msg_send![&*frame, addSubview: &*coat, positioned: ABOVE, relativeTo: &*effect]
        };
        Some(Self { effect, coat })
    }

    pub(super) fn tint(&self, material: isize, [red, green, blue]: [u8; 3], alpha: f64) {
        let part = |value: u8| f64::from(value) / 255.0;
        // SAFETY: an NSVisualEffectView takes an NSInteger material.
        let _: () = unsafe { msg_send![&*self.effect, setMaterial: material] };
        // SAFETY: NSColor's class method takes four CGFloats and returns a colour.
        let colour: Retained<AnyObject> = unsafe {
            msg_send![class!(NSColor),
                colorWithSRGBRed: part(red), green: part(green), blue: part(blue), alpha: alpha]
        };
        // SAFETY: an NSBox takes an NSColor fill, which it retains.
        let _: () = unsafe { msg_send![&*self.coat, setFillColor: &*colour] };
    }
}

/// A new view of `class`, as large as `bounds` and sized with its parent, named `name`.
fn view(class: &objc2::runtime::AnyClass, bounds: CGRect, name: &AnyObject) -> Retained<AnyObject> {
    // SAFETY: `class` is an NSView subclass, and `new` returns a retained instance.
    let view: Retained<AnyObject> = unsafe { msg_send![class, new] };
    // SAFETY: an NSView takes an NSRect frame, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*view, setFrame: bounds] };
    // SAFETY: an NSView takes an NSAutoresizingMaskOptions (an NSUInteger).
    let _: () = unsafe { msg_send![&*view, setAutoresizingMask: SIZABLE] };
    // SAFETY: an NSView takes an NSString identifier, which it copies.
    let _: () = unsafe { msg_send![&*view, setIdentifier: name] };
    view
}

/// Our materials in `window`'s frame view, the first one's state, whether it is opaque, the
/// shape its resizing keeps, and whether a coat is over them.
pub(super) fn dressed(window: &Window) -> (usize, isize, bool, f64, bool) {
    let native = window.ns_window() as *mut AnyObject;
    // SAFETY: `native` is tao's live NSWindow; `isOpaque` answers a BOOL.
    let opaque: Bool = unsafe { msg_send![native, isOpaque] };
    // SAFETY: an NSWindow answers `contentAspectRatio` with an NSSize, which `CGSize` lays out.
    let shape: CGSize = unsafe { msg_send![native, contentAspectRatio] };
    let aspect = if shape.height > 0.0 {
        shape.width / shape.height
    } else {
        0.0
    };
    let Some(frame) = frame_view(window) else {
        return (0, -1, opaque.as_bool(), aspect, false);
    };
    let (mut found, mut coated) = ((0, -1), false);
    // SAFETY: an NSView answers `subviews` with an NSArray.
    let views: Retained<AnyObject> = unsafe { msg_send![&*frame, subviews] };
    // SAFETY: an NSArray answers `count` with an NSUInteger.
    let count: usize = unsafe { msg_send![&*views, count] };
    for at in 0..count {
        // SAFETY: `at` is below the array's count, read just above on the same thread.
        let view: Retained<AnyObject> = unsafe { msg_send![&*views, objectAtIndex: at] };
        // SAFETY: an NSView answers `identifier` with an NSString or nil.
        let named: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, identifier] };
        let is = |wanted: &AnyObject| -> bool {
            named.as_ref().is_some_and(|named| {
                // SAFETY: both are NSStrings; the comparison answers a BOOL.
                let same: Bool = unsafe { msg_send![&**named, isEqualToString: wanted] };
                same.as_bool()
            })
        };
        coated |= is(&coat_name());
        if is(&name()) {
            // SAFETY: a view named ours is the NSVisualEffectView made above; `state` answers
            // an NSInteger.
            let state: isize = unsafe { msg_send![&*view, state] };
            found = (found.0 + 1, if found.0 == 0 { state } else { found.1 });
        }
    }
    (found.0, found.1, opaque.as_bool(), aspect, coated)
}

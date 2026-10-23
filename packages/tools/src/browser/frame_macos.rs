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

/// A window that shows what is behind it, with its titlebar clear over the material.
pub fn clear(builder: WindowBuilder) -> WindowBuilder {
    builder
        .with_transparent(true)
        .with_titlebar_transparent(true)
}

/// Whether the person asked for less transparency (Accessibility, Display).
pub fn reduced() -> bool {
    unsafe {
        let workspace: Retained<AnyObject> = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduced: Bool = msg_send![&*workspace, accessibilityDisplayShouldReduceTransparency];
        reduced.as_bool()
    }
}

/// What names our material among the frame view's own (the system puts materials there too).
fn name() -> Retained<AnyObject> {
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c"llx.glass".as_ptr()] }
}

/// The view that holds the titlebar and the content.
fn frame_view(window: &Window) -> Option<Retained<AnyObject>> {
    let window = window.ns_window() as *mut AnyObject;
    unsafe {
        let content: Option<Retained<AnyObject>> = msg_send![window, contentView];
        msg_send![&*content?, superview]
    }
}

/// A window's material, and a panel's coat over it. Kept, so a new tint changes these two and
/// never adds another.
pub struct Glass {
    effect: Retained<AnyObject>,
    coat: Option<Retained<AnyObject>>,
}

impl Glass {
    /// The material under everything in `window`, coated when `coated`.
    pub fn under(window: &Window, coated: bool) -> Option<Self> {
        let frame = frame_view(window)?;
        unsafe {
            let bounds: CGRect = msg_send![&*frame, bounds];
            let effect: Retained<AnyObject> = msg_send![class!(NSVisualEffectView), new];
            let _: () = msg_send![&*effect, setFrame: bounds];
            // Behind the window: the desktop, blurred.
            let _: () = msg_send![&*effect, setBlendingMode: 0isize];
            // Active, always: these windows are never key, and an inactive material stops
            // sampling and paints flat grey.
            let _: () = msg_send![&*effect, setState: 1isize];
            let _: () = msg_send![&*effect, setAutoresizingMask: SIZABLE];
            let _: () = msg_send![&*effect, setIdentifier: &*name()];
            let none: Option<&AnyObject> = None;
            let _: () =
                msg_send![&*frame, addSubview: &*effect, positioned: BELOW, relativeTo: none];
            let coat = coated.then(|| {
                let coat: Retained<AnyObject> = msg_send![class!(NSBox), new];
                // A custom box: a fill and nothing else, no border and no title.
                let _: () = msg_send![&*coat, setBoxType: 4usize];
                let _: () = msg_send![&*coat, setTitlePosition: 0usize];
                let _: () = msg_send![&*coat, setBorderWidth: 0.0f64];
                let _: () = msg_send![&*coat, setFrame: bounds];
                let _: () = msg_send![&*coat, setAutoresizingMask: SIZABLE];
                let _: () =
                    msg_send![&*frame, addSubview: &*coat, positioned: ABOVE, relativeTo: &*effect];
                coat
            });
            Some(Self { effect, coat })
        }
    }

    pub fn tint(&self, material: isize, [red, green, blue]: [u8; 3], alpha: f64) {
        let part = |value: u8| f64::from(value) / 255.0;
        unsafe {
            let _: () = msg_send![&*self.effect, setMaterial: material];
            if let Some(coat) = &self.coat {
                let colour: Retained<AnyObject> = msg_send![class!(NSColor),
                    colorWithSRGBRed: part(red), green: part(green), blue: part(blue), alpha: alpha];
                let _: () = msg_send![&**coat, setFillColor: &*colour];
            }
        }
    }
}

/// The person's resizing keeps the content's shape, `width` by `height`.
pub fn keep_shape(window: &Window, width: f64, height: f64) {
    let shape = CGSize { width, height };
    let _: () =
        unsafe { msg_send![window.ns_window() as *mut AnyObject, setContentAspectRatio: shape] };
}

/// Our materials in `window`'s frame view, the first one's state, whether it is opaque, and the
/// shape its resizing keeps.
pub fn dressed(window: &Window) -> (usize, isize, bool, f64) {
    let native = window.ns_window() as *mut AnyObject;
    let opaque: Bool = unsafe { msg_send![native, isOpaque] };
    let shape: CGSize = unsafe { msg_send![native, contentAspectRatio] };
    let aspect = if shape.height > 0.0 {
        shape.width / shape.height
    } else {
        0.0
    };
    let Some(frame) = frame_view(window) else {
        return (0, -1, opaque.as_bool(), aspect);
    };
    let mut found = (0, -1);
    unsafe {
        let views: Retained<AnyObject> = msg_send![&*frame, subviews];
        let count: usize = msg_send![&*views, count];
        for at in 0..count {
            let view: Retained<AnyObject> = msg_send![&*views, objectAtIndex: at];
            let named: Option<Retained<AnyObject>> = msg_send![&*view, identifier];
            let ours: Bool = match &named {
                Some(named) => msg_send![&**named, isEqualToString: &*name()],
                None => Bool::NO,
            };
            if ours.as_bool() {
                let state: isize = msg_send![&*view, state];
                found = (found.0 + 1, if found.0 == 0 { state } else { found.1 });
            }
        }
    }
    (found.0, found.1, opaque.as_bool(), aspect)
}

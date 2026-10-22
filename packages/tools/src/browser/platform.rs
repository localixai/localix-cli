//! What differs by platform: where a headless window goes, keeping it rendering, the user agent.

use std::sync::mpsc::Sender;

use tao::window::WindowBuilder;

#[cfg(not(target_os = "macos"))]
use super::host::{HEIGHT, WIDTH};

/// Where a headless window goes. macOS keeps it on screen and makes it invisible afterwards
/// ([`keep_rendering`]): WebKit marks an off-screen window hidden and stops its animation frames
/// and timers, so a page there would never settle. Elsewhere it goes off screen.
#[cfg(target_os = "macos")]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    builder
}

#[cfg(not(target_os = "macos"))]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    builder.with_position(tao::dpi::LogicalPosition::new(-WIDTH * 4.0, -HEIGHT * 4.0))
}

/// WebKit stops animation frames and timers in a window it considers hidden, and a driven window
/// usually sits behind the terminal that started it. So the window floats above other windows
/// (without taking focus), and occlusion detection, a WKWebView switch, is off as well. A headless
/// window is also fully transparent and passes clicks through: nothing covers it, nobody sees it.
#[cfg(target_os = "macos")]
pub(super) fn keep_rendering(webview: &wry::WebView, headless: bool) {
    use objc2::runtime::Bool;
    use objc2::{msg_send, sel};
    use wry::WebViewExtMacOS;

    const FLOATING_WINDOW_LEVEL: isize = 3;
    // NSWindowCollectionBehavior: open in the Space the person is in, even over a full-screen app,
    // so a new window never switches Spaces under them; a headless one also stays out of
    // window cycling and Mission Control.
    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const MOVE_TO_ACTIVE_SPACE: usize = 1 << 1;
    const TRANSIENT: usize = 1 << 3;
    const IGNORES_CYCLE: usize = 1 << 6;
    const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    let view = webview.webview();
    unsafe {
        let switch = sel!(_setWindowOcclusionDetectionEnabled:);
        let available: bool = msg_send![&*view, respondsToSelector: switch];
        if available {
            let _: () = msg_send![&*view, _setWindowOcclusionDetectionEnabled: Bool::NO];
        }
    }
    let window = webview.ns_window();
    unsafe {
        // Shown or not, the window floats: a covered page stops rendering. It never takes focus.
        let behavior = if headless {
            CAN_JOIN_ALL_SPACES | TRANSIENT | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY
        } else {
            MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_AUXILIARY
        };
        let _: () = msg_send![&*window, setCollectionBehavior: behavior];
        let _: () = msg_send![&*window, setLevel: FLOATING_WINDOW_LEVEL];
        let _: () = msg_send![&*window, orderFrontRegardless];
        if headless {
            let _: () = msg_send![&*window, setAlphaValue: 0.0f64];
            let _: () = msg_send![&*window, setIgnoresMouseEvents: Bool::YES];
            let _: () = msg_send![&*window, setHasShadow: Bool::NO];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_rendering(_webview: &wry::WebView, _headless: bool) {}

/// A PNG of what the WebView shows, sent to `reply` once WebKit has drawn it: WKWebView's own
/// snapshot, which works in an invisible window too. Called on the main thread.
#[cfg(target_os = "macos")]
pub(super) fn capture(webview: &wry::WebView, reply: Sender<Result<Vec<u8>, String>>) {
    use std::sync::Mutex;

    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use wry::WebViewExtMacOS;

    /// NSBitmapImageFileTypePNG.
    const PNG: usize = 4;
    let reply = Mutex::new(Some(reply));
    let done = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
        let Some(reply) = reply.lock().unwrap().take() else {
            return;
        };
        if image.is_null() {
            let _ = reply.send(Err("WebKit took no snapshot".into()));
            return;
        }
        let bytes = unsafe {
            let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
            let rep: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
            let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
            let png: *mut AnyObject =
                msg_send![rep, representationUsingType: PNG, properties: properties];
            if png.is_null() {
                None
            } else {
                let length: usize = msg_send![png, length];
                let start: *const u8 = msg_send![png, bytes];
                Some(std::slice::from_raw_parts(start, length).to_vec())
            }
        };
        let _ = reply.send(bytes.ok_or_else(|| "the snapshot could not be encoded".into()));
    });
    let view = webview.webview();
    unsafe {
        let _: () = msg_send![
            &*view,
            takeSnapshotWithConfiguration: std::ptr::null::<AnyObject>(),
            completionHandler: &*done
        ];
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capture(_webview: &wry::WebView, reply: Sender<Result<Vec<u8>, String>>) {
    let _ = reply.send(Err("screenshots are taken on macOS only".into()));
}

/// The installed Safari's user agent. A plain WKWebView omits the `Version/… Safari/…` tokens,
/// and sites such as Google then serve a legacy page.
#[cfg(target_os = "macos")]
pub(super) fn user_agent() -> Option<String> {
    let plist = std::fs::read_to_string("/Applications/Safari.app/Contents/Info.plist").ok();
    let version = plist
        .as_deref()
        .and_then(|plist| plist.split("<key>CFBundleShortVersionString</key>").nth(1))
        .and_then(|rest| rest.split("<string>").nth(1))
        .and_then(|rest| rest.split("</string>").next())
        .filter(|version| !version.is_empty() && version.len() < 16)
        .unwrap_or("26.0");
    Some(format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/{version} Safari/605.1.15"
    ))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn user_agent() -> Option<String> {
    None
}

//! What differs by platform: where a headless window goes, keeping it rendering, the Dock, the user agent.

use tao::event_loop::EventLoopWindowTarget;
use tao::window::WindowBuilder;

/// Whether the app is in the Dock while its window is shown, so a click on its icon brings the
/// window back from another desktop. A headless window has no icon: the app stays an accessory.
/// Showing the icon never activates the app (the window still never takes the keyboard).
#[cfg(target_os = "macos")]
pub(super) fn dock<T>(target: &EventLoopWindowTarget<T>, shown: bool) {
    use tao::platform::macos::{ActivationPolicy, EventLoopWindowTargetExtMacOS};
    let policy = match shown {
        true => ActivationPolicy::Regular,
        false => ActivationPolicy::Accessory,
    };
    target.set_activation_policy_at_runtime(policy);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn dock<T>(_target: &EventLoopWindowTarget<T>, _shown: bool) {}

/// Where a headless window goes. macOS keeps it on screen and makes it invisible afterwards
/// ([`keep_rendering`]): WebKit marks an off-screen window hidden and stops its animation frames
/// and timers, so a page there would never settle. Elsewhere it goes off screen.
#[cfg(target_os = "macos")]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    builder
}

#[cfg(not(target_os = "macos"))]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    let size = super::viewport::Viewport::DESKTOP;
    builder.with_position(tao::dpi::LogicalPosition::new(
        -size.width * 4.0,
        -size.height * 4.0,
    ))
}

/// WebKit stops animation frames and timers in a window it considers hidden, and a driven window
/// usually sits behind the terminal that started it. So occlusion detection, a WKWebView switch,
/// is off. A shown window is an ordinary one: it comes to the front once, without taking focus,
/// and goes behind other windows like any other. A headless window floats, in case the switch is
/// missing, but it is fully transparent and passes clicks through: it covers nothing.
#[cfg(target_os = "macos")]
pub(super) fn keep_rendering(webview: &wry::WebView, headless: bool) {
    use objc2::msg_send;
    use objc2::runtime::Bool;
    use wry::WebViewExtMacOS;

    // NSWindowCollectionBehavior: open in the Space the person is in, even over a full-screen app,
    // so a new window never switches Spaces under them; a headless one also stays out of
    // window cycling and Mission Control.
    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const MOVE_TO_ACTIVE_SPACE: usize = 1 << 1;
    const TRANSIENT: usize = 1 << 3;
    const IGNORES_CYCLE: usize = 1 << 6;
    const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    keep_tab_rendering(webview);
    let window = webview.ns_window();
    // It never takes focus.
    let behavior = if headless {
        CAN_JOIN_ALL_SPACES | TRANSIENT | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY
    } else {
        MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_AUXILIARY
    };
    // SAFETY: wry's NSWindow is alive while its WebView is; it takes an
    // NSWindowCollectionBehavior (an NSUInteger).
    let _: () = unsafe { msg_send![&*window, setCollectionBehavior: behavior] };
    // NSWindow levels: only an invisible window floats (3), above every ordinary one (0).
    let level: isize = if headless { 3 } else { 0 };
    // SAFETY: as above; an NSWindowLevel is an NSInteger.
    let _: () = unsafe { msg_send![&*window, setLevel: level] };
    // SAFETY: as above; it takes no argument and returns nothing.
    let _: () = unsafe { msg_send![&*window, orderFrontRegardless] };
    if headless {
        // SAFETY: as above; it takes a CGFloat.
        let _: () = unsafe { msg_send![&*window, setAlphaValue: 0.0f64] };
        // SAFETY: as above; it takes a BOOL.
        let _: () = unsafe { msg_send![&*window, setIgnoresMouseEvents: Bool::YES] };
        // SAFETY: as above; it takes a BOOL.
        let _: () = unsafe { msg_send![&*window, setHasShadow: Bool::NO] };
    }
}

/// A page keeps drawing while covered: WebKit's occlusion detection is off for its WebView.
#[cfg(target_os = "macos")]
pub(super) fn keep_tab_rendering(webview: &wry::WebView) {
    use objc2::runtime::Bool;
    use objc2::{msg_send, sel};
    use wry::WebViewExtMacOS;
    let view = webview.webview();
    let switch = sel!(_setWindowOcclusionDetectionEnabled:);
    // SAFETY: the WKWebView is a live NSObject; `respondsToSelector:` takes a selector and
    // answers a BOOL.
    let available: bool = unsafe { msg_send![&*view, respondsToSelector: switch] };
    if available {
        // SAFETY: the view answers this private selector (checked just above); it takes a BOOL.
        let _: () = unsafe { msg_send![&*view, _setWindowOcclusionDetectionEnabled: Bool::NO] };
    }
    // WebKit decides whether the page is shown when the view joins its window, with the switch
    // still on: a window that opened unseen (the screen asleep, another Space) stayed hidden,
    // its animation frames stopped. Hidden and shown again, it looks once more.
    // SAFETY: an NSView takes a BOOL `hidden`.
    let _: () = unsafe { msg_send![&*view, setHidden: Bool::YES] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*view, setHidden: Bool::NO] };
}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_tab_rendering(_webview: &wry::WebView) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_rendering(_webview: &wry::WebView, _headless: bool) {}

/// The installed Safari's user agent. A plain WKWebView omits the `Version/… Safari/…` tokens,
/// and sites such as Google then serve a legacy page.
#[cfg(target_os = "macos")]
#[allow(
    clippy::unnecessary_wraps,
    reason = "one signature on every system: elsewhere there is none"
)]
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

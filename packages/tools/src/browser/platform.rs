//! What differs by platform: where a headless window goes, keeping it rendering, the Dock, the user agent.

use tao::event_loop::EventLoopWindowTarget;
use tao::window::WindowBuilder;

/// With `LLX_WINDOWS=unseen` (the tests', AGENTS.md) a shown window is everything it is, strip,
/// glass and panels, but on screen it is what a headless one is: clear, passing clicks through,
/// with no Dock icon, so a test run never covers what the person is watching (macOS).
#[cfg(target_os = "macos")]
fn unseen() -> bool {
    std::env::var_os("LLX_WINDOWS").is_some_and(|value| value == "unseen")
}

/// Whether the app is in the Dock while its window is shown, so a click on its icon brings the
/// window back from another desktop. A headless window has no icon: the app stays an accessory.
/// Showing the icon never activates the app (the window still never takes the keyboard).
#[cfg(target_os = "macos")]
pub(super) fn dock<T>(target: &EventLoopWindowTarget<T>, shown: bool) {
    use tao::platform::macos::{ActivationPolicy, EventLoopWindowTargetExtMacOS};
    let policy = match shown && !unseen() {
        true => ActivationPolicy::Regular,
        false => ActivationPolicy::Accessory,
    };
    target.set_activation_policy_at_runtime(policy);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn dock<T>(_target: &EventLoopWindowTarget<T>, _shown: bool) {}

/// Start the visible deck in macOS's native full-screen Space. Test windows stay unseen.
#[cfg(target_os = "macos")]
pub(super) fn fullscreen(window: &tao::window::Window) {
    if !unseen() {
        window.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn fullscreen(_window: &tao::window::Window) {}

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
/// is off. A shown window is ordered behind other windows without taking focus; macOS places it
/// in an ordinary Space rather than moving it onto a full-screen app. A headless window is fully
/// transparent, passes clicks through, and goes behind every window, as Bun's and WebKit's own
/// test windows do: on screen, so WebKit sees it shown and the display it is on stays awake,
/// under whatever the person watches. Only where the switch is missing does it float, so that
/// nothing covers it.
#[cfg(target_os = "macos")]
pub(super) fn keep_rendering(webview: &wry::WebView, headless: bool) {
    use objc2::msg_send;
    use objc2::runtime::Bool;
    use wry::WebViewExtMacOS;

    // A shown window participates in ordinary Spaces and can itself go full screen. A headless
    // one joins every Space, even over a full-screen app, and stays out of window cycling.
    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const MANAGED: usize = 1 << 2;
    const TRANSIENT: usize = 1 << 3;
    const IGNORES_CYCLE: usize = 1 << 6;
    const FULL_SCREEN_PRIMARY: usize = 1 << 7;
    const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    let switched = keep_tab_rendering(webview);
    let headless = headless || unseen();
    let window = webview.ns_window();
    // It never takes focus.
    let behavior = if headless {
        CAN_JOIN_ALL_SPACES | TRANSIENT | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY
    } else {
        MANAGED | FULL_SCREEN_PRIMARY
    };
    // SAFETY: wry's NSWindow is alive while its WebView is; it takes an
    // NSWindowCollectionBehavior (an NSUInteger).
    let _: () = unsafe { msg_send![&*window, setCollectionBehavior: behavior] };
    // NSWindow levels: an invisible window floats (3), above every ordinary one (0), only when
    // nothing else keeps it drawing.
    let level: isize = if headless && !switched { 3 } else { 0 };
    // SAFETY: as above; an NSWindowLevel is an NSInteger.
    let _: () = unsafe { msg_send![&*window, setLevel: level] };
    // A headless window is invisible before it is ever on screen: made clear after it came to the
    // front, it flashed over whatever the person was watching, once for every `exec` and test.
    if headless {
        // SAFETY: as above; it takes a CGFloat.
        let _: () = unsafe { msg_send![&*window, setAlphaValue: 0.0f64] };
        // SAFETY: as above; it takes a BOOL.
        let _: () = unsafe { msg_send![&*window, setIgnoresMouseEvents: Bool::YES] };
        // SAFETY: as above; it takes a BOOL.
        let _: () = unsafe { msg_send![&*window, setHasShadow: Bool::NO] };
    }
    if !headless || switched {
        // SAFETY: as above; `orderBack:` takes a sender, which may be nil, and returns nothing.
        let _: () = unsafe {
            msg_send![&*window, orderBack: std::ptr::null::<objc2::runtime::AnyObject>()]
        };
    } else {
        // SAFETY: as above; it takes no argument and returns nothing.
        let _: () = unsafe { msg_send![&*window, orderFrontRegardless] };
    }
}

/// A page keeps drawing while covered: WebKit's occlusion detection is off for its WebView.
/// Whether it is: the switch is private, and a later WebKit may not have it.
#[cfg(target_os = "macos")]
pub(super) fn keep_tab_rendering(webview: &wry::WebView) -> bool {
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
    available
}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_tab_rendering(_webview: &wry::WebView) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_rendering(_webview: &wry::WebView, _headless: bool) {}

/// The installed Safari's user agent. A plain WKWebView omits the `Version/… Safari/…` tokens,
/// and sites such as Google then serve a legacy page.
#[cfg(target_os = "macos")]
#[expect(
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

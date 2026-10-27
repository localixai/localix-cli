//! The top row of a tab as WebKit drew it, as pixels: a thin band across its width, taken small,
//! for the strip's tint (`tint.rs`). macOS only; elsewhere no pixels, and the strip keeps its own
//! colours.

use super::super::tint::Rgb;

/// The band's height, in points: under a page's top edge, above most of its words.
#[cfg(target_os = "macos")]
const BAND: f64 = 6.0;
/// The band's width once drawn, in points: a pixel every few of a wide page is enough.
#[cfg(target_os = "macos")]
const WIDE: f64 = 24.0;
/// How many of the band's pixels are read, across and down.
#[cfg(target_os = "macos")]
const COLUMNS: isize = 64;
#[cfg(target_os = "macos")]
const ROWS: isize = 3;

/// Draw the top band of `webview` and hand its pixels to `done`, on the main thread, later.
#[cfg(target_os = "macos")]
pub(in super::super) fn top(webview: &wry::WebView, done: impl FnOnce(Vec<Rgb>) + 'static) {
    use std::sync::Mutex;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use wry::WebViewExtMacOS;

    use super::{CGPoint, CGRect, CGSize};

    let view = webview.webview();
    // SAFETY: the WKWebView is an NSView; `bounds` answers an NSRect, which `CGRect` lays out.
    let bounds: CGRect = unsafe { msg_send![&*view, bounds] };
    let rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: bounds.size.width,
            height: BAND.min(bounds.size.height),
        },
    };
    // SAFETY: `new` on WKSnapshotConfiguration returns a retained instance.
    let configuration: Retained<AnyObject> =
        unsafe { msg_send![class!(WKSnapshotConfiguration), new] };
    // SAFETY: a snapshot configuration takes an NSRect, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*configuration, setRect: rect] };
    // SAFETY: NSNumber's class method takes a double and returns a number.
    let points: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNumber), numberWithDouble: WIDE] };
    // SAFETY: a snapshot configuration takes an NSNumber width, which it retains.
    let _: () = unsafe { msg_send![&*configuration, setSnapshotWidth: &*points] };
    // What is on screen now: waiting for the next screen update would hold the switch up.
    // SAFETY: a snapshot configuration takes a BOOL.
    let _: () = unsafe { msg_send![&*configuration, setAfterScreenUpdates: Bool::NO] };
    // WebKit calls the block once; the slot hands `done` to that one call.
    let done = Mutex::new(Some(done));
    let block = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
        let pixels = if image.is_null() {
            vec![]
        } else {
            // SAFETY: WebKit hands a live NSImage for the length of the callback, and what
            // `read` makes of it is autoreleased into this pool.
            objc2::rc::autoreleasepool(|_| unsafe { read(image) })
        };
        let done = crate::lock(&done).take();
        if let Some(done) = done {
            done(pixels);
        }
    });
    // SAFETY: a WKWebView takes a configuration and a block of (NSImage, NSError), which it
    // copies and calls once on the main thread.
    let _: () = unsafe {
        msg_send![
            &*view,
            takeSnapshotWithConfiguration: &*configuration,
            completionHandler: &*block
        ]
    };
}

/// Every pixel of the NSImage WebKit drew, in sRGB.
///
/// # Safety
/// `image` is a live NSImage, and an autorelease pool is in place.
#[cfg(target_os = "macos")]
unsafe fn read(image: *mut objc2::runtime::AnyObject) -> Vec<Rgb> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    // SAFETY: `image` is a live NSImage (the caller's contract); the data is autoreleased.
    let tiff: *mut AnyObject = unsafe { msg_send![image, TIFFRepresentation] };
    if tiff.is_null() {
        return vec![];
    }
    // SAFETY: `tiff` is a live NSData, checked non-null above.
    let bitmap: *mut AnyObject =
        unsafe { msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff] };
    if bitmap.is_null() {
        return vec![];
    }
    // SAFETY: NSColorSpace's class method answers the shared sRGB space.
    let srgb: *mut AnyObject = unsafe { msg_send![class!(NSColorSpace), sRGBColorSpace] };
    // SAFETY: `bitmap` is a live NSBitmapImageRep; it answers NSIntegers.
    let width: isize = unsafe { msg_send![bitmap, pixelsWide] };
    // SAFETY: as above.
    let height: isize = unsafe { msg_send![bitmap, pixelsHigh] };
    let channel = |value: f64| {
        let scaled = (value.clamp(0.0, 1.0) * 255.0).round();
        // The nearest level found, not cut by `as`.
        (0..=u8::MAX)
            .rev()
            .find(|&level| f64::from(level) <= scaled)
            .unwrap_or(0)
    };
    let mut pixels = vec![];
    // A few rows and columns are the band's colour: WebKit may hand the band at full size
    // (thousands of pixels), and every pixel read is a message.
    let step = |of: isize, wanted: isize| usize::try_from((of / wanted).max(1)).unwrap_or(1);
    for y in (0..height).step_by(step(height, ROWS)) {
        for x in (0..width).step_by(step(width, COLUMNS)) {
            // SAFETY: `bitmap` is live and (x, y) inside it; the colour is autoreleased.
            let colour: *mut AnyObject = unsafe { msg_send![bitmap, colorAtX: x, y: y] };
            if colour.is_null() {
                continue;
            }
            // SAFETY: `colour` is a live NSColor, `srgb` a live NSColorSpace.
            let colour: *mut AnyObject = unsafe { msg_send![colour, colorUsingColorSpace: srgb] };
            if colour.is_null() {
                continue;
            }
            // SAFETY: an sRGB NSColor answers its components as CGFloats.
            let r: f64 = unsafe { msg_send![colour, redComponent] };
            // SAFETY: as above.
            let g: f64 = unsafe { msg_send![colour, greenComponent] };
            // SAFETY: as above.
            let b: f64 = unsafe { msg_send![colour, blueComponent] };
            pixels.push(Rgb {
                r: channel(r),
                g: channel(g),
                b: channel(b),
            });
        }
    }
    pixels
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn top(_webview: &wry::WebView, done: impl FnOnce(Vec<Rgb>) + 'static) {
    done(vec![]);
}

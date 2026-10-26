//! A picture of the page as the WebView drew it: the compositor's own pixels (a canvas, WebGL and
//! video included), not the DOM drawn again. JPEG, at most `MAX_EDGE` pixels on its long side, so
//! a screenshot costs the model about as much as a long page outline.

use std::sync::mpsc::SyncSender;

/// The long side of a picture, in pixels.
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the compositor's picture is taken only on macOS; its tests use it everywhere"
    )
)]
pub(super) const MAX_EDGE: f64 = 1280.0;
/// JPEG quality: text stays legible, a full page stays near 150 KB.
#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "the compositor's picture is taken only on macOS")
)]
const QUALITY: f64 = 0.7;

/// A part of the page, in CSS pixels from the viewport's top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// An encoded picture.
#[derive(Debug, Clone)]
pub struct Shot {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The size to draw `width`×`height` at so its long side is at most `MAX_EDGE`.
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(
        dead_code,
        reason = "the compositor's picture is taken only on macOS; its tests use it everywhere"
    )
)]
pub(super) fn fitted(width: f64, height: f64) -> (f64, f64) {
    let scale = (MAX_EDGE / width.max(height)).min(1.0);
    ((width * scale).round(), (height * scale).round())
}

/// Take the picture of `clip` (or of the whole view) and send it to `reply` once WebKit has drawn
/// it. Runs on the main thread, where the WebView lives; the answer comes later, from there too.
#[cfg(target_os = "macos")]
pub(super) fn capture(
    webview: &wry::WebView,
    clip: Option<Clip>,
    reply: SyncSender<Result<Shot, String>>,
) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use wry::WebViewExtMacOS;

    let view = webview.webview();
    let window = webview.ns_window();
    // SAFETY: the WKWebView is an NSView; `bounds` answers an NSRect, which `CGRect` lays out.
    let bounds: CGRect = unsafe { msg_send![&*view, bounds] };
    // SAFETY: wry's NSWindow is alive while its WebView is; it answers a CGFloat.
    let scale: f64 = unsafe { msg_send![&*window, backingScaleFactor] };
    let rect = match clip {
        Some(clip) => CGRect {
            origin: CGPoint {
                x: clip.x,
                y: clip.y,
            },
            size: CGSize {
                width: clip.width,
                height: clip.height,
            },
        },
        None => bounds,
    };
    // The snapshot's width is in points; the bitmap has the screen's scale on top of it.
    let (width, _) = fitted(rect.size.width * scale, rect.size.height * scale);
    // SAFETY: `new` on WKSnapshotConfiguration returns a retained instance.
    let configuration: Retained<AnyObject> =
        unsafe { msg_send![class!(WKSnapshotConfiguration), new] };
    // SAFETY: a snapshot configuration takes an NSRect, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*configuration, setRect: rect] };
    // SAFETY: NSNumber's class method takes a double and returns a number.
    let points: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNumber), numberWithDouble: width / scale.max(1.0)] };
    // SAFETY: a snapshot configuration takes an NSNumber width, which it retains.
    let _: () = unsafe { msg_send![&*configuration, setSnapshotWidth: &*points] };
    // SAFETY: a snapshot configuration takes a BOOL.
    let _: () = unsafe { msg_send![&*configuration, setAfterScreenUpdates: Bool::YES] };
    let done = RcBlock::new(move |image: *mut AnyObject, error: *mut AnyObject| {
        let shot = if image.is_null() {
            Err(match error.is_null() {
                true => "WebKit returned no picture".to_string(),
                // SAFETY: WebKit hands a live NSError for the length of the callback.
                false => unsafe { describe(error) },
            })
        } else {
            // SAFETY: WebKit hands a live NSImage for the length of the callback, and what
            // `encode` reads of it is autoreleased into this pool.
            objc2::rc::autoreleasepool(|_| unsafe { encode(image) })
        };
        let _ = reply.send(shot);
    });
    // SAFETY: a WKWebView takes a configuration and a block of (NSImage, NSError), which it
    // copies and calls once on the main thread.
    let _: () = unsafe {
        msg_send![
            &*view,
            takeSnapshotWithConfiguration: &*configuration,
            completionHandler: &*done
        ]
    };
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capture(
    _webview: &wry::WebView,
    _clip: Option<Clip>,
    reply: SyncSender<Result<Shot, String>>,
) {
    let _ = reply.send(Err(
        "screenshots are not supported on this system yet (macOS only); use `read` or `script`"
            .into(),
    ));
}

/// The NSImage WebKit drew, as JPEG.
///
/// # Safety
/// `image` is a live NSImage, and an autorelease pool is in place.
#[cfg(target_os = "macos")]
unsafe fn encode(image: *mut objc2::runtime::AnyObject) -> Result<Shot, String> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    /// NSBitmapImageFileTypeJPEG.
    const JPEG: usize = 3;
    // SAFETY: `image` is a live NSImage (the caller's contract); the data is autoreleased.
    let tiff: *mut AnyObject = unsafe { msg_send![image, TIFFRepresentation] };
    if tiff.is_null() {
        return Err("the picture has no bitmap".into());
    }
    // SAFETY: `tiff` is a live NSData, checked non-null above.
    let bitmap: *mut AnyObject =
        unsafe { msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff] };
    if bitmap.is_null() {
        return Err("the picture could not be decoded".into());
    }
    // SAFETY: the argument is a NUL-terminated UTF-8 literal, alive for the whole program.
    let key: *mut AnyObject = unsafe {
        msg_send![
            class!(NSString),
            stringWithUTF8String: c"NSImageCompressionFactor".as_ptr()
        ]
    };
    // SAFETY: NSNumber's class method takes a double and returns a number.
    let quality: *mut AnyObject = unsafe { msg_send![class!(NSNumber), numberWithDouble: QUALITY] };
    // SAFETY: `quality` and `key` are live objects, made just above in this pool.
    let properties: *mut AnyObject =
        unsafe { msg_send![class!(NSDictionary), dictionaryWithObject: quality, forKey: key] };
    // SAFETY: `bitmap` is a live NSBitmapImageRep (non-null above), `properties` a dictionary.
    let data: *mut AnyObject =
        unsafe { msg_send![bitmap, representationUsingType: JPEG, properties: properties] };
    if data.is_null() {
        return Err("the picture could not be encoded as JPEG".into());
    }
    // SAFETY: `data` is a live NSData, checked non-null above.
    let bytes: *const std::ffi::c_void = unsafe { msg_send![data, bytes] };
    // SAFETY: as above; an NSData answers its length as an NSUInteger.
    let length: usize = unsafe { msg_send![data, length] };
    // SAFETY: `bitmap` is a live NSBitmapImageRep; it answers an NSInteger.
    let width: isize = unsafe { msg_send![bitmap, pixelsWide] };
    // SAFETY: as above.
    let height: isize = unsafe { msg_send![bitmap, pixelsHigh] };
    // SAFETY: `bytes` points at `length` bytes the NSData owns, alive while the pool is; they
    // are copied out before it drains.
    let jpeg = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length) }.to_vec();
    Ok(Shot {
        jpeg,
        width: u32::try_from(width.max(0)).unwrap_or(u32::MAX),
        height: u32::try_from(height.max(0)).unwrap_or(u32::MAX),
    })
}

/// An NSError's description.
///
/// # Safety
/// `error` is a live NSError.
#[cfg(target_os = "macos")]
unsafe fn describe(error: *mut objc2::runtime::AnyObject) -> String {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    // SAFETY: `error` is a live NSError (the caller's contract); it answers an NSString.
    let text: *mut AnyObject = unsafe { msg_send![error, localizedDescription] };
    // SAFETY: `text` is a live NSString; its UTF-8 lives as long as it does, or is null.
    let utf8: *const std::ffi::c_char = unsafe { msg_send![text, UTF8String] };
    if utf8.is_null() {
        return "WebKit could not take the picture".into();
    }
    // SAFETY: `utf8` is non-null and NUL-terminated, alive while `text` is, and copied out here.
    unsafe { std::ffi::CStr::from_ptr(utf8) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[cfg(target_os = "macos")]
mod encoding {
    use objc2::{Encode, Encoding};

    use super::{CGPoint, CGRect, CGSize};

    // SAFETY: `CGPoint` is `repr(C)`, two f64s, as the encoding says.
    unsafe impl Encode for CGPoint {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    // SAFETY: `CGSize` is `repr(C)`, two f64s, as the encoding says.
    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    // SAFETY: `CGRect` is `repr(C)`, a `CGPoint` then a `CGSize`, as the encoding says.
    unsafe impl Encode for CGRect {
        const ENCODING: Encoding =
            Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

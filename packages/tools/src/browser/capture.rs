//! A picture of the page as the WebView drew it: the compositor's own pixels (a canvas, WebGL and
//! video included), not the DOM drawn again. JPEG, at most `MAX_EDGE` pixels on its long side, so
//! a screenshot costs the model about as much as a long page outline.

use std::sync::mpsc::Sender;

/// The long side of a picture, in pixels.
pub(super) const MAX_EDGE: f64 = 1280.0;
/// JPEG quality: text stays legible, a full page stays near 150 KB.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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
    reply: Sender<Result<Shot, String>>,
) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use wry::WebViewExtMacOS;

    let view = webview.webview();
    let window = webview.ns_window();
    let bounds: CGRect = unsafe { msg_send![&*view, bounds] };
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
    unsafe {
        let configuration: Retained<AnyObject> = msg_send![class!(WKSnapshotConfiguration), new];
        let _: () = msg_send![&*configuration, setRect: rect];
        let points: Retained<AnyObject> =
            msg_send![class!(NSNumber), numberWithDouble: width / scale.max(1.0)];
        let _: () = msg_send![&*configuration, setSnapshotWidth: &*points];
        let _: () = msg_send![&*configuration, setAfterScreenUpdates: Bool::YES];
        let done = RcBlock::new(move |image: *mut AnyObject, error: *mut AnyObject| {
            let shot = if image.is_null() {
                Err(match error.is_null() {
                    true => "WebKit returned no picture".to_string(),
                    false => describe(error),
                })
            } else {
                objc2::rc::autoreleasepool(|_| encode(image))
            };
            let _ = reply.send(shot);
        });
        let _: () = msg_send![
            &*view,
            takeSnapshotWithConfiguration: &*configuration,
            completionHandler: &*done
        ];
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capture(
    _webview: &wry::WebView,
    _clip: Option<Clip>,
    reply: Sender<Result<Shot, String>>,
) {
    let _ = reply.send(Err(
        "screenshots are not supported on this system yet (macOS only); use `read` or `script`"
            .into(),
    ));
}

/// The NSImage WebKit drew, as JPEG.
#[cfg(target_os = "macos")]
unsafe fn encode(image: *mut objc2::runtime::AnyObject) -> Result<Shot, String> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    /// NSBitmapImageFileTypeJPEG.
    const JPEG: usize = 3;
    unsafe {
        let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
        if tiff.is_null() {
            return Err("the picture has no bitmap".into());
        }
        let bitmap: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
        if bitmap.is_null() {
            return Err("the picture could not be decoded".into());
        }
        let key: *mut AnyObject = msg_send![
            class!(NSString),
            stringWithUTF8String: c"NSImageCompressionFactor".as_ptr()
        ];
        let quality: *mut AnyObject = msg_send![class!(NSNumber), numberWithDouble: QUALITY];
        let properties: *mut AnyObject =
            msg_send![class!(NSDictionary), dictionaryWithObject: quality, forKey: key];
        let data: *mut AnyObject =
            msg_send![bitmap, representationUsingType: JPEG, properties: properties];
        if data.is_null() {
            return Err("the picture could not be encoded as JPEG".into());
        }
        let bytes: *const std::ffi::c_void = msg_send![data, bytes];
        let length: usize = msg_send![data, length];
        let width: isize = msg_send![bitmap, pixelsWide];
        let height: isize = msg_send![bitmap, pixelsHigh];
        Ok(Shot {
            jpeg: std::slice::from_raw_parts(bytes.cast::<u8>(), length).to_vec(),
            width: width.max(0) as u32,
            height: height.max(0) as u32,
        })
    }
}

/// An NSError's description.
#[cfg(target_os = "macos")]
unsafe fn describe(error: *mut objc2::runtime::AnyObject) -> String {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let text: *mut AnyObject = msg_send![error, localizedDescription];
        let utf8: *const std::ffi::c_char = msg_send![text, UTF8String];
        if utf8.is_null() {
            return "WebKit could not take the picture".into();
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned()
    }
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

    unsafe impl Encode for CGPoint {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for CGRect {
        const ENCODING: Encoding =
            Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

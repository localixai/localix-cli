//! The picture on Linux: WebKitGTK's own snapshot of what the view shows (its compositor's
//! pixels, as on macOS), cut to the clip, made smaller to `MAX_EDGE` and encoded by gdk-pixbuf,
//! all libraries the WebView already brings.

use std::sync::mpsc::SyncSender;

use gtk::gdk_pixbuf::{InterpType, Pixbuf};
use gtk::prelude::WidgetExt;
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
use wry::WebViewExtUnix;

use super::{Clip, QUALITY, Shot, fitted};

/// Take the picture of `clip` (or of the whole view) and send it to `reply` once WebKit has drawn
/// it. Runs on the main thread, where GTK and the WebView live; the answer comes from there too.
pub(in super::super) fn capture(
    webview: &wry::WebView,
    clip: Option<Clip>,
    reply: SyncSender<Result<Shot, String>>,
) {
    let view = webview.webview();
    let shown = f64::from(view.allocated_width().max(1));
    view.snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&gtk::gio::Cancellable>,
        move |drawn| {
            let shot = drawn
                .map_err(|error| error.to_string())
                .and_then(|surface| encode(&surface, shown, clip));
            let _ = reply.send(shot);
        },
    );
}

/// The surface WebKit drew, cut to `clip` (CSS pixels of a view `shown` wide), as JPEG.
fn encode(surface: &gtk::cairo::Surface, shown: f64, clip: Option<Clip>) -> Result<Shot, String> {
    let image = gtk::cairo::ImageSurface::try_from(surface.clone()).map_err(|surface| {
        format!(
            "WebKit's picture is a {:?} surface, not an image",
            surface.type_()
        )
    })?;
    let (width, height) = (image.width(), image.height());
    // The surface is in device pixels: the screen's scale on top of the page's CSS pixels.
    let scale = f64::from(width) / shown;
    let (x, y, w, h) = match clip {
        Some(clip) => (clip.x, clip.y, clip.width, clip.height),
        None => (
            0.0,
            0.0,
            f64::from(width) / scale,
            f64::from(height) / scale,
        ),
    };
    let pixel = |value: f64, most: i32| pixels(value * scale).clamp(0, most);
    let (left, top) = (pixel(x, width - 1), pixel(y, height - 1));
    let (right, bottom) = (pixel(x + w, width), pixel(y + h, height));
    let pixbuf = gtk::gdk::pixbuf_get_from_surface(
        surface,
        left,
        top,
        (right - left).max(1),
        (bottom - top).max(1),
    )
    .ok_or("the picture could not be read")?;
    let (fit_w, fit_h) = fitted(f64::from(pixbuf.width()), f64::from(pixbuf.height()));
    let small: Pixbuf = pixbuf
        .scale_simple(
            pixels(fit_w).max(1),
            pixels(fit_h).max(1),
            InterpType::Bilinear,
        )
        .ok_or("the picture could not be made smaller")?;
    let quality = format!("{}", pixels(QUALITY * 100.0));
    let jpeg = small
        .save_to_bufferv("jpeg", &[("quality", quality.as_str())])
        .map_err(|error| format!("the picture could not be encoded as JPEG: {error}"))?;
    Ok(Shot {
        jpeg,
        width: u32::try_from(small.width()).unwrap_or(0),
        height: u32::try_from(small.height()).unwrap_or(0),
    })
}

/// A length in whole pixels.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a picture's side is far below i32::MAX, and rounded first"
)]
fn pixels(value: f64) -> i32 {
    value.round() as i32
}

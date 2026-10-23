use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::WindowEvent;

use super::*;

fn rect(rect: wry::Rect) -> (f64, f64, f64, f64) {
    let at = rect.position.to_logical::<f64>(1.0);
    let size = rect.size.to_logical::<f64>(1.0);
    (at.x, at.y, size.width, size.height)
}

#[test]
fn a_page_fills_its_window_wherever_it_stands() {
    let size = LogicalSize::new(1200.0, 800.0);
    assert_eq!(rect(fill(size, 0.0)), (0.0, 0.0, 1200.0, 800.0));
    // A tab out of sight keeps the window's size.
    assert_eq!(rect(fill(size, -1300.0)), (-1300.0, 0.0, 1200.0, 800.0));
}

#[test]
fn a_new_size_or_scale_lays_the_pages_out_again() {
    assert!(relayout(&WindowEvent::Resized(PhysicalSize::new(800, 600))));
    let mut size = PhysicalSize::new(1600, 1200);
    let scaled = WindowEvent::ScaleFactorChanged {
        scale_factor: 2.0,
        new_inner_size: &mut size,
    };
    assert!(relayout(&scaled));
    assert!(!relayout(&WindowEvent::Moved(PhysicalPosition::new(1, 2))));
    assert!(!relayout(&WindowEvent::Focused(true)));
}

#[test]
fn glass_is_for_a_shown_window_on_macos() {
    assert_eq!(look(true, true), Look::Hidden);
    assert_eq!(look(true, false), Look::Hidden);
    let shown = if cfg!(target_os = "macos") {
        Look::Glass
    } else {
        Look::Opaque
    };
    assert_eq!(look(false, true), shown);
    // Reduced transparency: an ordinary window.
    assert_eq!(look(false, false), Look::Opaque);
}

#[test]
fn a_ground_is_dark_below_mid_grey() {
    assert!(dark([0x1e, 0x1e, 0x1e]));
    assert!(dark([0x7f, 0x7f, 0x7f]));
    assert!(!dark([0x80, 0x80, 0x80]));
    assert!(!dark([0xff, 0xff, 0xff]));
    // Luma, not the mean: a saturated blue is dark, a yellow light.
    assert!(dark([0x20, 0x40, 0xff]));
    assert!(!dark([0xff, 0xd7, 0x00]));
}

#[test]
fn each_appearance_has_its_own_material_and_coat() {
    // HUD over a dark ground; under-window background, which reads light, over a light one.
    assert_eq!(material(true), 13);
    assert_eq!(material(false), 21);
    let (dark_ground, dark_alpha) = coat(None, true);
    let (light_ground, light_alpha) = coat(None, false);
    assert_eq!((dark_ground, light_ground), (DARK, LIGHT));
    assert!(dark_alpha < light_alpha && dark_alpha >= 0.8 && light_alpha < 1.0);
    // The person's own ground, whatever the system's appearance.
    assert_eq!(coat(Some([1, 2, 3]), false), ([1, 2, 3], dark_alpha));
}

#[test]
fn a_page_is_dressed_in_the_frames_stylesheet() {
    let page = "<!doctype html>\n<html>\n<head>\n<title>t</title></head><body></body></html>";
    let glass = dress(page, Look::Glass);
    assert!(glass.contains("<html data-glass>"), "{glass}");
    assert!(
        glass.contains(&format!("<head><style>{CSS}</style>")),
        "{glass}"
    );
    let opaque = dress(page, Look::Opaque);
    assert!(opaque.contains("<html>") && opaque.contains(CSS));
    assert_eq!(opaque.matches("<style>").count(), 1);
    assert!(CSS.contains("prefers-reduced-transparency"));
}

#[test]
fn a_pages_shape_is_two_sane_numbers() {
    assert_eq!(shape("[968,708]"), Some((968.0, 708.0)));
    assert_eq!(shape("[968.4, 708.6]"), Some((969.0, 709.0)));
    // Whatever the page says, the window stays a window.
    assert_eq!(shape("[1e9,5]"), Some((2400.0, 240.0)));
    for answer in ["null", "\"x\"", "[1]", "[1,\"2\"]", "{}", ""] {
        assert_eq!(shape(answer), None, "{answer}");
    }
}

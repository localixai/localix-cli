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
    assert_eq!(rect(fill(size, 0.0, 0.0)), (0.0, 0.0, 1200.0, 800.0));
    // A tab out of sight keeps the window's size.
    assert_eq!(
        rect(fill(size, -1300.0, 34.0)),
        (-1300.0, 34.0, 1200.0, 800.0)
    );
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
    assert_eq!(look(true, Transparency::Allowed), Look::Hidden);
    assert_eq!(look(true, Transparency::Reduced), Look::Hidden);
    let shown = if cfg!(target_os = "macos") {
        Look::Glass
    } else {
        Look::Opaque
    };
    assert_eq!(look(false, Transparency::Allowed), shown);
    // Reduced transparency: an ordinary window.
    assert_eq!(look(false, Transparency::Reduced), Look::Opaque);
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
    // Mostly glass: at most 60% coat, lighter where the text is dark.
    assert!(dark_alpha < light_alpha && dark_alpha >= 0.4 && light_alpha <= 0.6);
    // A neutral glass: a terminal's own grey-blue ground only says which
    // appearance; the coat stays near-black (or near-white), so the material does not read grey.
    let neutral = |[red, green, blue]: Rgb| red == green && green == blue;
    let (over_grey_blue, alpha) = coat(Some([0x2a, 0x2b, 0x37]), false);
    assert!(
        neutral(over_grey_blue) && over_grey_blue[0] < 0x10,
        "{over_grey_blue:?}"
    );
    assert_eq!(alpha, dark_alpha);
    let (over_paper, _) = coat(Some([0xee, 0xee, 0xe0]), true);
    assert!(
        neutral(over_paper) && over_paper[0] > 0xf0,
        "{over_paper:?}"
    );
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
fn a_shown_window_opens_at_a_size_fit_for_its_screen() {
    // A laptop's screen: a good part of it, not all of it.
    assert_eq!(moderate((1512.0, 945.0)), (960.0, 680.0));
    // A large screen: no larger than reads well.
    assert_eq!(moderate((3008.0, 1692.0)), (1440.0, 960.0));
    // A small one: never smaller than a page needs.
    assert_eq!(moderate((1024.0, 700.0)), (960.0, 640.0));
}

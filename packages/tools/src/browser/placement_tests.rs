use super::*;

const LAPTOP: Area = Area {
    x: 0.0,
    y: 38.0,
    width: 1512.0,
    height: 870.0,
};

#[test]
fn the_window_opens_on_the_right_of_the_screen_and_fills_its_height() {
    let at = deck(LAPTOP, None);
    // About 60% of the width, the right edge a margin from the screen's.
    assert_eq!((at.width, at.height), (960.0, 846.0));
    assert_eq!(at.x + at.width, LAPTOP.width - MARGIN);
    assert_eq!(at.y, LAPTOP.y + MARGIN);
}

#[test]
fn a_large_screen_gets_no_more_than_reads_well_and_a_small_one_what_a_page_needs() {
    let big = Area {
        width: 3008.0,
        height: 1600.0,
        ..LAPTOP
    };
    assert_eq!(deck(big, None).width, 1440.0);
    let wide = Area {
        width: 2000.0,
        ..LAPTOP
    };
    assert_eq!(deck(wide, None).width, 1200.0);
    let small = Area {
        width: 1024.0,
        height: 700.0,
        ..LAPTOP
    };
    assert_eq!(deck(small, None).width, 960.0);
    // Never wider than the screen itself.
    let tiny = Area {
        width: 800.0,
        ..LAPTOP
    };
    assert_eq!(deck(tiny, None).width, 800.0 - 2.0 * MARGIN);
}

#[test]
fn a_frame_the_person_gave_it_is_kept() {
    let kept = Rect {
        x: 100.0,
        y: 120.0,
        width: 700.0,
        height: 500.0,
    };
    assert_eq!(deck(LAPTOP, Some(kept)), kept);
}

#[test]
fn the_mini_opens_in_the_bottom_right_corner() {
    let (x, y) = mini(LAPTOP, None);
    assert_eq!(x + MINI.0, LAPTOP.width - CORNER);
    assert_eq!(y + MINI.1, LAPTOP.y + LAPTOP.height - CORNER);
}

#[test]
fn where_the_person_put_the_mini_is_kept_while_it_is_on_the_screen() {
    assert_eq!(mini(LAPTOP, Some((40.0, 100.0))), (40.0, 100.0));
    // A screen that went away: the corner's edge, not off it.
    assert_eq!(
        mini(LAPTOP, Some((5000.0, -300.0))),
        (LAPTOP.width - MINI.0, LAPTOP.y)
    );
}

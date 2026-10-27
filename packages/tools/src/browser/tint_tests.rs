use super::*;

const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const NAVY: Rgb = Rgb {
    r: 20,
    g: 30,
    b: 70,
};

fn band(colours: &[(Rgb, usize)]) -> Vec<Rgb> {
    colours
        .iter()
        .flat_map(|&(colour, count)| std::iter::repeat_n(colour, count))
        .collect()
}

#[test]
fn a_top_of_one_colour_tints_the_strip() {
    assert_eq!(dominant(&band(&[(NAVY, 100)])), Some(NAVY));
}

#[test]
fn a_few_other_pixels_a_logo_or_text_do_not_change_it() {
    let logo = Rgb {
        r: 250,
        g: 70,
        b: 0,
    };
    assert_eq!(
        dominant(&band(&[(NAVY, 80), (logo, 12), (WHITE, 8)])),
        Some(NAVY)
    );
}

#[test]
fn jpeg_noise_around_a_colour_is_the_colour() {
    let near = Rgb {
        r: 22,
        g: 29,
        b: 71,
    };
    let got = dominant(&band(&[(NAVY, 50), (near, 50)])).unwrap();
    assert!(
        got.r.abs_diff(21) <= 1 && got.b.abs_diff(70) <= 1,
        "{got:?}"
    );
}

#[test]
fn a_grey_on_both_sides_of_a_round_number_is_one_colour() {
    let (below, above) = (
        Rgb {
            r: 127,
            g: 127,
            b: 127,
        },
        Rgb {
            r: 128,
            g: 128,
            b: 128,
        },
    );
    assert!(dominant(&band(&[(below, 50), (above, 50)])).is_some());
}

#[test]
fn a_band_of_a_wide_screen_at_full_size_is_read_at_once() {
    let wide = band(&[(NAVY, 20_000), (WHITE, 3_000)]);
    let began = std::time::Instant::now();
    assert_eq!(dominant(&wide), Some(NAVY));
    assert!(
        began.elapsed() < std::time::Duration::from_millis(50),
        "{:?}",
        began.elapsed()
    );
}

#[test]
fn a_busy_top_a_picture_or_a_banner_leaves_the_strip_as_it_is() {
    let red = Rgb { r: 200, g: 0, b: 0 };
    let green = Rgb { r: 0, g: 160, b: 0 };
    assert_eq!(dominant(&band(&[(NAVY, 40), (red, 30), (green, 30)])), None);
    assert_eq!(dominant(&[]), None);
}

#[test]
fn the_strip_writes_in_the_ink_that_reads_on_its_ground() {
    assert_eq!(ink(WHITE), Ink::Dark);
    assert_eq!(ink(NAVY), Ink::Light);
    assert_eq!(
        ink(Rgb {
            r: 255,
            g: 210,
            b: 0
        }),
        Ink::Dark
    );
    assert_eq!(
        ink(Rgb {
            r: 30,
            g: 30,
            b: 30
        }),
        Ink::Light
    );
}

#[test]
fn a_tab_seen_before_is_tinted_at_once_and_drawn_once_at_a_time() {
    let mut memory = Memory::default();
    assert_eq!(memory.recall(&1), None);
    assert!(memory.start(1));
    assert!(!memory.start(1), "one picture of a tab at a time");
    assert!(memory.start(2));
    memory.remember(1, Some(NAVY));
    memory.remember(2, None);
    assert_eq!(memory.recall(&1), Some(Seen::Tinted(NAVY)));
    assert_eq!(memory.recall(&2), Some(Seen::Busy));
    assert!(
        memory.start(1),
        "drawn again once the last picture came back"
    );
}

#[test]
fn memory_keeps_only_the_latest_tabs() {
    let mut memory = Memory::default();
    for tab in 0..100 {
        memory.remember(tab, Some(NAVY));
    }
    assert_eq!(memory.recall(&0), None);
    assert_eq!(memory.recall(&99), Some(Seen::Tinted(NAVY)));
}

#[test]
fn the_strip_is_told_its_ground_and_ink_or_to_go_back_to_its_own() {
    assert_eq!(
        script(Some(NAVY)),
        r##"T({"ground":"#141e46","ink":"light"})"##
    );
    assert_eq!(script(None), "T(null)");
}

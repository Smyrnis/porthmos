use super::*;

#[test]
fn the_scale_grows_in_strict_steps_on_a_four_pixel_grid() {
    let scale = [XS, SM, MD, LG, XL];

    assert!(scale.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(scale.iter().all(|step| step % 4.0 == 0.0));
}

#[test]
fn the_scale_matches_what_the_settings_window_already_uses() {
    assert_eq!((XS, SM, MD, LG, XL), (4.0, 8.0, 12.0, 16.0, 24.0));
}

#[test]
fn the_dialog_padding_is_the_large_step() {
    assert_eq!(DIALOG_PADDING, LG);
}

#[test]
fn a_dialog_stacks_its_parts_one_medium_step_apart() {
    assert_eq!(DIALOG_GAP, MD);
}

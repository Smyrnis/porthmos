use super::size_text;

#[test]
fn bytes_then_one_decimal_units_with_a_space() {
    assert_eq!(size_text(0), "0 B");
    assert_eq!(size_text(22), "22 B");
    assert_eq!(size_text(1023), "1023 B");
    assert_eq!(size_text(1024), "1.0 KB");
    assert_eq!(size_text(2457), "2.4 KB");
    assert_eq!(size_text(13_312_000), "12.7 MB");
    assert_eq!(size_text(50 * 1024 * 1024), "50.0 MB");
    assert_eq!(size_text(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    assert_eq!(size_text(2 * 1024u64.pow(4)), "2.0 TB");
}

#[test]
fn a_value_rolls_over_to_the_next_unit_before_it_needs_nine_cells() {
    assert_eq!(size_text(1_022_976), "999.0 KB");
    assert_eq!(size_text(1_023_999), "1.0 MB");
    assert_eq!(size_text(1_048_575), "1.0 MB");
    assert_eq!(size_text(1_073_741_000), "1.0 GB");
}

#[test]
fn the_largest_value_stays_in_the_largest_unit() {
    assert_eq!(size_text(u64::MAX), "16777216.0 TB");
}

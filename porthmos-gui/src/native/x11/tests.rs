use super::packed_position;

#[test]
fn the_pointer_position_is_packed_with_x_in_the_high_half() {
    assert_eq!(packed_position(0x12, 0x34), 0x0012_0034);
}

#[test]
fn a_negative_coordinate_keeps_its_sixteen_bit_pattern() {
    assert_eq!(packed_position(-1, 5), 0xffff_0005);
}

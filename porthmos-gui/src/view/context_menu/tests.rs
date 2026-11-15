use iced::{Point, Size};

use super::clamp_menu;

#[test]
fn the_menu_position_is_clamped_inside_the_window() {
    let menu = Size::new(200.0, 100.0);
    let window = Size::new(1000.0, 700.0);

    assert_eq!(clamp_menu(Point::new(10.0, 20.0), menu, window), Point::new(10.0, 20.0));
    assert_eq!(clamp_menu(Point::new(950.0, 650.0), menu, window), Point::new(800.0, 600.0));
    assert_eq!(clamp_menu(Point::new(-5.0, -5.0), menu, window), Point::new(0.0, 0.0));
}

#[test]
fn a_menu_bigger_than_the_window_sits_at_the_origin() {
    let clamped = clamp_menu(Point::new(50.0, 50.0), Size::new(500.0, 500.0), Size::new(300.0, 300.0));

    assert_eq!(clamped, Point::new(0.0, 0.0));
}

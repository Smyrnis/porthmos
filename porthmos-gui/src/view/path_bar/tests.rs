use porthmos_core::VfsPath;

use super::visible_crumbs;
use crate::breadcrumbs::crumbs;

#[test]
fn trailing_crumbs_are_kept_when_the_bar_is_narrow() {
    let all = crumbs(&VfsPath::new("/home/user/projects/porthmos"));

    let (clipped, shown) = visible_crumbs(&all, 20);

    assert!(clipped);
    assert_eq!(shown.last().unwrap().label, "porthmos");
    assert!(shown.len() < all.len());
}

#[test]
fn every_crumb_is_kept_when_there_is_room() {
    let all = crumbs(&VfsPath::new("/a/b"));

    let (clipped, shown) = visible_crumbs(&all, 80);

    assert!(!clipped);
    assert_eq!(shown.len(), all.len());
}

#[test]
fn the_last_crumb_is_kept_even_when_it_alone_is_too_long() {
    let all = crumbs(&VfsPath::new("/a/an_extremely_long_folder_name"));

    let (clipped, shown) = visible_crumbs(&all, 5);

    assert!(clipped);
    assert_eq!(shown.len(), 1);
}

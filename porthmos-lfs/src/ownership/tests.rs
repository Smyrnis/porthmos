use super::*;

#[test]
fn user_zero_is_root() {
    assert_eq!(user_name(0), "root");
}

#[test]
fn an_unknown_user_id_is_shown_as_the_number() {
    assert_eq!(user_name(4_000_000_001), "4000000001");
}

#[test]
fn an_unknown_group_id_is_shown_as_the_number() {
    assert_eq!(group_name(4_000_000_001), "4000000001");
}

#[test]
fn the_current_user_has_a_name() {
    let uid = unsafe { libc::geteuid() };

    let name = user_name(uid);

    assert!(!name.is_empty());
}

#[test]
fn the_current_group_has_a_name() {
    let gid = unsafe { libc::getegid() };

    let name = group_name(gid);

    assert!(!name.is_empty());
}

#[test]
fn group_zero_is_the_administrators_group() {
    assert!(["root", "wheel"].contains(&group_name(0).as_str()));
}

#[test]
fn a_buffer_that_is_too_small_is_doubled_until_the_name_fits() {
    let sizes = std::cell::RefCell::new(Vec::new());

    let name = resolve(7, |buffer| {
        sizes.borrow_mut().push(buffer.len());
        if buffer.len() < 4096 { Lookup::TooSmall } else { Lookup::Found("big".to_string()) }
    });

    assert_eq!(name, "big");
    assert_eq!(*sizes.borrow(), vec![1024, 2048, 4096]);
}

#[test]
fn a_name_that_never_fits_falls_back_to_the_number() {
    let name = resolve(7, |_| Lookup::TooSmall);

    assert_eq!(name, "7");
}

#[test]
fn an_interrupted_lookup_is_retried() {
    let calls = std::cell::Cell::new(0);

    let name = resolve(7, |_| {
        calls.set(calls.get() + 1);
        if calls.get() < 3 { Lookup::Interrupted } else { Lookup::Found("nikos".to_string()) }
    });

    assert_eq!(name, "nikos");
    assert_eq!(calls.get(), 3);
}

#[test]
fn a_lookup_that_is_always_interrupted_gives_up_with_the_number() {
    let name = resolve(7, |_| Lookup::Interrupted);

    assert_eq!(name, "7");
}

#[test]
fn an_empty_name_falls_back_to_the_number() {
    assert_eq!(resolve(7, |_| Lookup::Found(String::new())), "7");
}

#[test]
fn a_missing_entry_falls_back_to_the_number() {
    assert_eq!(resolve(7, |_| Lookup::Missing), "7");
}

#[test]
fn the_name_cache_remembers_each_id() {
    let mut cache = NameCache::new(user_name);

    assert_eq!(cache.name(0), "root");
    assert_eq!(cache.name(0), "root");
}

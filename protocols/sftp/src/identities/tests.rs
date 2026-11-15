use russh_sftp::protocol::FileAttributes;

use super::{Identities, parse_names};

const PASSWD: &str = "root:x:0:0:root:/root:/bin/bash\nnikos:x:1000:1000:Nikos:/home/nikos:/bin/zsh\n";
const GROUP: &str = "root:x:0:\nstaff:x:20:nikos,bob\n";

fn attributes(uid: Option<u32>, gid: Option<u32>) -> FileAttributes {
    let mut attributes = FileAttributes::empty();
    attributes.uid = uid;
    attributes.gid = gid;
    attributes
}

#[test]
fn a_table_maps_the_third_field_to_the_first() {
    let names = parse_names(PASSWD);

    assert_eq!(names.get(&0).map(String::as_str), Some("root"));
    assert_eq!(names.get(&1000).map(String::as_str), Some("nikos"));
    assert_eq!(names.len(), 2);
}

#[test]
fn comments_blank_and_malformed_lines_are_skipped() {
    let names = parse_names("# note\n\nbroken\nname:x:notanumber:0\n:x:5:0\nok:x:7:0\n");

    assert_eq!(names.len(), 1);
    assert_eq!(names.get(&7).map(String::as_str), Some("ok"));
}

#[test]
fn the_first_entry_for_an_id_wins() {
    let names = parse_names("first:x:5:5\nsecond:x:5:5\n");

    assert_eq!(names.get(&5).map(String::as_str), Some("first"));
}

#[test]
fn ids_are_turned_into_the_names_from_the_tables() {
    let identities = Identities::from_tables(PASSWD, GROUP);

    let owned = identities.ownership(&attributes(Some(1000), Some(20)));

    assert_eq!(owned, (Some("nikos".to_string()), Some("staff".to_string())));
}

#[test]
fn the_server_supplied_names_win_over_the_tables() {
    let identities = Identities::from_tables(PASSWD, GROUP);
    let mut given = attributes(Some(1000), Some(20));
    given.user = Some("alias".to_string());
    given.group = Some("crew".to_string());

    assert_eq!(identities.ownership(&given), (Some("alias".to_string()), Some("crew".to_string())));
}

#[test]
fn an_id_missing_from_the_tables_stays_numeric() {
    let identities = Identities::from_tables(PASSWD, GROUP);

    let owned = identities.ownership(&attributes(Some(30033), Some(30034)));

    assert_eq!(owned, (Some("30033".to_string()), Some("30034".to_string())));
}

#[test]
fn without_tables_numeric_ids_stand_in() {
    let owned = Identities::default().ownership(&attributes(Some(501), Some(20)));

    assert_eq!(owned, (Some("501".to_string()), Some("20".to_string())));
}

#[test]
fn an_empty_server_name_falls_back_to_the_table_then_the_id() {
    let identities = Identities::from_tables(PASSWD, GROUP);
    let mut given = attributes(Some(1000), Some(77));
    given.user = Some(String::new());
    given.group = Some(String::new());

    assert_eq!(identities.ownership(&given), (Some("nikos".to_string()), Some("77".to_string())));
}

#[test]
fn owner_and_group_are_unknown_without_names_or_ids() {
    let identities = Identities::from_tables(PASSWD, GROUP);

    assert_eq!(identities.ownership(&FileAttributes::empty()), (None, None));
}

#[test]
fn users_and_groups_use_separate_tables() {
    let identities = Identities::from_tables("alice:x:20:20\n", "staff:x:20:\n");

    let owned = identities.ownership(&attributes(Some(20), Some(20)));

    assert_eq!(owned, (Some("alice".to_string()), Some("staff".to_string())));
}

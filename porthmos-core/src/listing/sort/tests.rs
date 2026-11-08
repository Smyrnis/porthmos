use porthmos_vfs::VfsPath;

use porthmos_vfs::Entry;

use super::*;

fn entry(name: &str, is_dir: bool, size: u64) -> Entry {
    Entry {
        modified: None,
        name: name.to_string(),
        path: VfsPath::from(format!("/{name}")),
        is_dir,
        size,
        permissions: None,
        ..Default::default()
    }
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn directories_sort_before_files_regardless_of_key() {
    let mut entries = vec![entry("b_file.txt", false, 10), entry("a_dir", true, 0)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Size, order: SortOrder::Descending });
    assert_eq!(names(&entries), vec!["a_dir", "b_file.txt"]);
}

#[test]
fn name_ascending_is_case_insensitive() {
    let mut entries = vec![entry("Banana", false, 0), entry("apple", false, 0)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Name, order: SortOrder::Ascending });
    assert_eq!(names(&entries), vec!["apple", "Banana"]);
}

#[test]
fn name_descending_reverses_order() {
    let mut entries = vec![entry("apple", false, 0), entry("banana", false, 0)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Name, order: SortOrder::Descending });
    assert_eq!(names(&entries), vec!["banana", "apple"]);
}

#[test]
fn size_ascending_orders_by_byte_count() {
    let mut entries = vec![entry("big.txt", false, 100), entry("small.txt", false, 1)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Size, order: SortOrder::Ascending });
    assert_eq!(names(&entries), vec!["small.txt", "big.txt"]);
}

#[test]
fn equal_sizes_tiebreak_ascending_by_name() {
    let mut entries = vec![entry("b.txt", false, 5), entry("a.txt", false, 5)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Size, order: SortOrder::Descending });
    assert_eq!(names(&entries), vec!["a.txt", "b.txt"]);
}

#[test]
fn cycled_advances_name_asc_desc_size_asc_desc_then_wraps() {
    let start = SortSpec::default();
    assert_eq!(start, SortSpec { key: SortKey::Name, order: SortOrder::Ascending });

    let s1 = start.cycled();
    assert_eq!(s1, SortSpec { key: SortKey::Name, order: SortOrder::Descending });

    let s2 = s1.cycled();
    assert_eq!(s2, SortSpec { key: SortKey::Size, order: SortOrder::Ascending });

    let s3 = s2.cycled();
    assert_eq!(s3, SortSpec { key: SortKey::Size, order: SortOrder::Descending });

    assert_eq!(s3.cycled(), start);
}

fn sorted_names(input: &[&str], spec: SortSpec) -> Vec<String> {
    let mut entries: Vec<Entry> = input.iter().map(|name| entry(name, false, 0)).collect();
    sort_entries(&mut entries, spec);
    entries.into_iter().map(|entry| entry.name).collect()
}

const BY_NAME: SortSpec = SortSpec { key: SortKey::Name, order: SortOrder::Ascending };

#[test]
fn numbers_in_names_sort_by_value_not_digit_by_digit() {
    assert_eq!(
        sorted_names(&["f1000", "f10", "f1", "f100", "f102", "f2"], BY_NAME),
        ["f1", "f2", "f10", "f100", "f102", "f1000"]
    );
}

#[test]
fn numbers_in_the_middle_of_a_name_sort_by_value() {
    assert_eq!(
        sorted_names(&["img10.png", "img9.png", "img9b.png", "img100.png"], BY_NAME),
        ["img9.png", "img9b.png", "img10.png", "img100.png"]
    );
}

#[test]
fn natural_order_still_ignores_case() {
    assert_eq!(sorted_names(&["File10", "file2", "FILE1"], BY_NAME), ["FILE1", "file2", "File10"]);
}

#[test]
fn descending_name_order_is_the_exact_reverse_of_natural_order() {
    let descending = SortSpec { key: SortKey::Name, order: SortOrder::Descending };
    assert_eq!(sorted_names(&["f1", "f10", "f2"], descending), ["f10", "f2", "f1"]);
}

#[test]
fn numbers_longer_than_any_integer_type_still_compare_by_value() {
    let big = "n99999999999999999999999999999999";
    let bigger = "n100000000000000000000000000000000";
    assert_eq!(sorted_names(&[bigger, big, "n5"], BY_NAME), ["n5", big, bigger]);
}

#[test]
fn leading_zeros_do_not_change_the_order_by_value_and_ties_are_stable() {
    assert_eq!(sorted_names(&["a10", "a02", "a1", "a002", "a01"], BY_NAME), ["a1", "a01", "a02", "a002", "a10"]);
    assert_eq!(sorted_names(&["a01", "a1"], BY_NAME), sorted_names(&["a1", "a01"], BY_NAME));
}

#[test]
fn a_name_that_is_a_prefix_of_another_comes_first() {
    assert_eq!(sorted_names(&["a10", "a", "a1"], BY_NAME), ["a", "a1", "a10"]);
}

#[test]
fn digits_sort_before_letters_and_plain_text_is_unchanged() {
    assert_eq!(sorted_names(&["b", "1", "a", "10", "2"], BY_NAME), ["1", "2", "10", "a", "b"]);
}

#[test]
fn equal_sizes_break_ties_in_natural_order() {
    let mut entries = vec![entry("f10", false, 5), entry("f2", false, 5)];
    sort_entries(&mut entries, SortSpec { key: SortKey::Size, order: SortOrder::Ascending });
    assert_eq!(names(&entries), vec!["f2", "f10"]);
}

#[test]
fn non_ascii_names_with_numbers_do_not_panic_and_order_by_value() {
    assert_eq!(sorted_names(&["é10", "é9", "ü2"], BY_NAME), ["é9", "é10", "ü2"]);
}

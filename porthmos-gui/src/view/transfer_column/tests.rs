use super::arrow_states;
use crate::testing::Harness;

#[test]
fn arrows_are_disabled_without_a_connection() {
    let harness = Harness::new();

    assert_eq!(arrow_states(&harness.app), (false, false));
}

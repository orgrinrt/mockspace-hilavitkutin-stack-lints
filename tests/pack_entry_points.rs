//! Smoke test: the pack's entry points return non-empty lint sets and the
//! macro-generated `lints()` / `cross_lints()` are callable.

use mockspace_hilavitkutin_stack_lints as pack;

#[test]
fn pack_exposes_lints_entry_point() {
    let lints = pack::lints();
    assert!(!lints.is_empty(), "lint pack should expose at least one lint");
}

#[test]
fn pack_exposes_cross_lints_entry_point() {
    let _ = pack::cross_lints();
}

#[test]
fn every_lint_has_a_name() {
    for lint in pack::lints() {
        let name = lint.name();
        assert!(!name.is_empty(), "lint name should not be empty");
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "lint name `{name}` should be kebab-case");
    }
}

#[test]
fn lint_names_are_unique() {
    let names: Vec<&'static str> = pack::lints().iter().map(|l| l.name()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(names.len(), sorted.len(), "lint names collide: {names:?}");
}

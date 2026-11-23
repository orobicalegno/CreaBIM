//! A test that fails on purpose, only on the branch volumen-prova-ci-fallita:
//! it shows that a build whose checks fail does not replace the preview.
//! This branch is never merged.

#[test]
fn fails_on_purpose_to_check_the_preview_stays() {
    panic!("this test fails on purpose: the preview must not be replaced");
}

//! Exercise the human demonstration without parsing Cargo harness arguments.

#[test]
fn human_demonstration_completes_the_faulty_and_corrected_demonstrations() {
    super::run().unwrap();
}

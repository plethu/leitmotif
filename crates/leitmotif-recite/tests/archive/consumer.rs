//! Exercise the real consumer entry point in Cargo's example test harness.

#[test]
fn consumer_entrypoint_completes_the_faulty_and_corrected_demonstrations() {
    super::main().unwrap();
}

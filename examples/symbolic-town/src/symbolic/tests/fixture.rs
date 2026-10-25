use clap::Parser;

use super::{Args, measure};

#[test]
fn measured_commands_cover_both_engines_families_orders_and_queries() {
    for backend in ["explicit", "oxidd"] {
        for (family, order) in [
            ("sparse", "paired"),
            ("sparse", "banked"),
            ("coupled", "paired"),
            ("coupled", "banked"),
        ] {
            for query in ["goal", "local", "unreachable"] {
                check_measurement(backend, family, order, query);
            }
        }
    }
}

fn check_measurement(backend: &str, family: &str, order: &str, query: &str) {
    let args = Args::try_parse_from([
        "symbolic_town",
        "--backend",
        backend,
        "--family",
        family,
        "--threads",
        "2",
        "--local-domain",
        "3",
        "--order",
        order,
        "--query",
        query,
    ])
    .unwrap();
    let result = measure(&args).unwrap();
    let expected = if query == "unreachable" || (query == "goal" && family == "coupled") {
        "unreachable"
    } else {
        "reached"
    };
    assert_eq!(result.result["conclusion"], expected);
    assert!(!result.model_id.is_empty());
    assert!(result.analysis_seconds >= 0.0);
    assert!(result.variables > 0);
}

#[test]
fn invalid_dimensions_and_resource_budgets_are_rejected() {
    for (argument, value) in [
        ("--max-depth", "257"),
        ("--node-capacity", "3"),
        ("--threads", "0"),
        ("--local-domain", "9"),
    ] {
        let args =
            Args::try_parse_from(["symbolic_town", "--backend", "oxidd", argument, value]).unwrap();
        assert!(measure(&args).is_err());
    }
    let args = Args::try_parse_from([
        "symbolic_town",
        "--backend",
        "oxidd",
        "--family",
        "coupled",
        "--threads",
        "0",
    ])
    .unwrap();
    assert!(measure(&args).is_err());
}

#[test]
fn explicit_measurement_preserves_inconclusive_depth_bound() {
    let args = Args::try_parse_from(["symbolic_town", "--backend", "explicit", "--max-depth", "0"])
        .unwrap();
    let result = measure(&args).unwrap();
    assert_eq!(result.result["conclusion"], "inconclusive");
}

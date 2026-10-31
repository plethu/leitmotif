"""The private experiment keeps the same strict gates as the main workspace."""

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXPERIMENT = ROOT / "examples/symbolic-town"


class SymbolicConfigurationTests(unittest.TestCase):
    def test_leaf_preserves_lints_and_owns_only_its_solver_graph(self):
        root = tomllib.loads((ROOT / "Cargo.toml").read_text())
        leaf = tomllib.loads((EXPERIMENT / "Cargo.toml").read_text())
        core = tomllib.loads((ROOT / "crates/leitmotif/Cargo.toml").read_text())
        self.assertIn("examples/symbolic-town", root["workspace"]["exclude"])
        self.assertEqual(leaf["lints"], root["workspace"]["lints"])
        self.assertFalse(leaf["package"]["publish"])
        self.assertNotIn("symbolic-experiment", core["features"])
        self.assertNotIn("oxidd", core["dependencies"])
        self.assertEqual(set(leaf["patch"]["crates-io"]), {"oxidd-derive"})
        self.assertNotIn("oxidd-derive", root.get("patch", {}).get("crates-io", {}))
        core_lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
        leaf_lock = tomllib.loads((EXPERIMENT / "Cargo.lock").read_text())
        self.assertFalse(
            any(p["name"].startswith("oxidd") for p in core_lock["package"])
        )
        self.assertFalse(
            any(p["name"].startswith("bevy") for p in leaf_lock["package"])
        )

    def test_leaf_policy_matches_root_without_unrelated_source_or_license_exceptions(
        self,
    ):
        root = tomllib.loads((ROOT / "deny.toml").read_text())
        leaf = tomllib.loads((EXPERIMENT / "deny.toml").read_text())
        for section in ["graph", "advisories"]:
            self.assertEqual(leaf[section], root[section])
        # The root's skips excuse duplicates in its own (Bevy) graph, not this one.
        self.assertNotIn("skip", leaf["bans"])
        self.assertEqual(
            leaf["bans"], {k: v for k, v in root["bans"].items() if k != "skip"}
        )
        leaf_licenses = leaf["licenses"].copy()
        self.assertEqual(
            leaf_licenses.pop("exceptions"),
            [{"name": "nanorand", "version": "=0.8.0", "allow": ["Zlib"]}],
        )
        self.assertEqual(
            leaf_licenses,
            {k: v for k, v in root["licenses"].items() if k != "exceptions"},
        )
        leaf_sources = leaf["sources"].copy()
        self.assertEqual(
            leaf_sources.pop("allow-git"), ["https://github.com/plethu/oxidd"]
        )
        self.assertEqual(
            leaf_sources, {k: v for k, v in root["sources"].items() if k != "allow-git"}
        )

    def test_root_and_ci_include_every_experiment_gate(self):
        just = (ROOT / "justfile").read_text()
        workflow = (ROOT / ".github/workflows/check.yml").read_text()
        lane = (ROOT / ".config/just/symbolic.just").read_text()
        self.assertIn("mod symbolic '.config/just/symbolic.just'", just)
        aggregate = just.split("\ncheck:\n", 1)[1].split("\n\n", 1)[0]
        self.assertIn("    just symbolic check", aggregate)
        rust_job = workflow.split("  rust:\n", 1)[1].split("  engine:\n", 1)[0]
        self.assertIn("- run: just symbolic check", rust_job)
        self.assertIn("check: core-check supply-chain coverage", lane)
        for command in [
            "cargo clippy",
            "cargo nextest run",
            "cargo build",
            "cargo doc",
            "cargo deny",
            "cargo machete",
            "llvm-cov nextest",
            "scripts/coverage_gate.py",
        ]:
            self.assertIn(command, lane)
        self.assertIn("--branch", lane)
        self.assertIn("--locked", lane)
        self.assertIn("python -m unittest discover -s tests/dependencies -v", just)

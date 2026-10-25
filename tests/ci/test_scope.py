"""CI must never skip code changes hidden by renames, history, or new inputs."""

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("ci_scope", ROOT / "scripts/ci_scope.py")
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class ScopeTests(unittest.TestCase):
    def test_only_known_documentation_can_skip_rust(self):
        self.assertFalse(SCOPE.needs_rust(["docs/guide.md", "CONTRIBUTING.md"]))
        for path in [
            "README.md",
            "crates/lib.rs",
            "Cargo.lock",
            "examples/symbolic-town/Cargo.toml",
            "examples/symbolic-town/Cargo.lock",
            "examples/symbolic-town/src/symbolic/search.rs",
            ".config/just/symbolic.just",
            ".mise.toml",
            ".github/workflows/check.yml",
            "new-input",
            "docs/example.rs",
        ]:
            with self.subTest(path=path):
                self.assertTrue(SCOPE.needs_rust(["docs/notes.md", path]))
        self.assertTrue(SCOPE.needs_rust([]))

    def test_events_without_change_history_run_everything(self):
        for event, base in [
            ("schedule", "abc"),
            ("workflow_dispatch", "abc"),
            ("push", "0" * 40),
            ("pull_request", ""),
        ]:
            with self.subTest(event=event, base=base):
                self.assertTrue(SCOPE.select_rust(event, base, "HEAD"))


class GitHistoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        previous = Path.cwd()
        os.chdir(self.directory.name)
        self.addCleanup(os.chdir, previous)
        self.git("init", "-b", "main")
        self.git("config", "user.name", "CI fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.hooksPath", "/dev/null")
        Path("source.rs").write_text("source", encoding="utf-8")
        Path("docs").mkdir()
        Path("docs/notes.md").write_text("notes", encoding="utf-8")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()

    @staticmethod
    def git(*arguments):
        return subprocess.run(
            ["git", *arguments], check=True, capture_output=True, text=True
        ).stdout

    def commit(self):
        self.git("add", "--all")
        self.git("commit", "-m", "fixture")

    def test_documentation_only_change_and_machine_output(self):
        Path("docs/notes.md").write_text("changed", encoding="utf-8")
        self.commit()
        self.assertFalse(SCOPE.select_rust("push", self.base, "HEAD"))
        destination = Path("output")
        result = subprocess.run(
            [
                "python",
                str(ROOT / "scripts/ci_scope.py"),
                "--event",
                "push",
                "--base",
                self.base,
            ],
            env={**os.environ, "GITHUB_OUTPUT": str(destination)},
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.stdout, "rust=false\n")
        self.assertEqual(destination.read_text(), result.stdout)

    def test_deleted_source_is_checked(self):
        Path("source.rs").unlink()
        self.commit()
        self.assertTrue(SCOPE.select_rust("push", self.base, "HEAD"))

    def test_rename_from_source_to_docs_is_checked(self):
        self.git("mv", "source.rs", "docs/source.md")
        self.commit()
        self.assertEqual(
            set(SCOPE.changed_paths(self.base, "HEAD")), {"source.rs", "docs/source.md"}
        )
        self.assertTrue(SCOPE.select_rust("push", self.base, "HEAD"))

    def test_pr_uses_merge_base_not_unrelated_target_changes(self):
        self.git("checkout", "-b", "docs-change")
        Path("docs/notes.md").write_text("branch edit", encoding="utf-8")
        self.commit()
        self.git("checkout", "main")
        Path("source.rs").write_text("target edit", encoding="utf-8")
        self.commit()
        self.assertFalse(SCOPE.select_rust("pull_request", "main", "docs-change"))
        self.assertTrue(SCOPE.select_rust("push", "main", "docs-change"))

    def test_missing_history_runs_everything(self):
        self.assertTrue(SCOPE.select_rust("push", "missing-ref", "HEAD"))
        self.assertTrue(SCOPE.select_rust("push", self.base, "HEAD"))


if __name__ == "__main__":
    unittest.main()

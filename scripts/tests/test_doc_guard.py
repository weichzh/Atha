"""Exercise the documentation gates against real, isolated Git histories."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
ACTIVE = "docs/ACTIVE.md"
CHANGE = "docs/changes/fixture.md"
ROUTE = f"- `change`：Fixture → `{CHANGE}`\n"
READER = "reader/app/layout.ts"
DOC = "docs/notes.md"


class DocGuardTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="atha-doc-guard-")
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name) / "repo with spaces"
        self.root.mkdir()
        bin_dir = Path(temp.name) / "bin"
        bin_dir.mkdir()
        mise = bin_dir / "mise"
        mise.write_text(
            """#!/usr/bin/env bash
if [[ $1 == exec && $2 == -- && $3 == python3 ]]; then
  shift 2
  exec "$@"
fi
printf 'Unexpected toolchain command: %s\\n' "$*" >&2
exit 99
""",
            encoding="utf-8",
        )
        mise.chmod(0o700)
        self.env = {
            **{key: value for key, value in os.environ.items() if not key.startswith("GIT_")},
            "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
            "LC_ALL": "C",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_AUTHOR_NAME": "Fixture",
            "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
            "GIT_COMMITTER_NAME": "Fixture",
            "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
        }
        (self.root / "scripts").mkdir()
        for name in (
            "doc_guard.py", "doc_length_check.py", "check-docs.sh",
            "check-workflow.sh", "check-fast.sh",
        ):
            shutil.copyfile(SCRIPTS / name, self.root / "scripts" / name)

        self.git("init", "-q", "--template=")
        self.write(".gitignore", "/artifacts/local/\n")
        self.write(ACTIVE, "# Active\n\n" + ROUTE)
        self.write(CHANGE, """# Fixture

## Status

accepted

## Problem

## Scope

## Architecture Impact

none

## Acceptance Criteria

## Files And Steps

## Checks

## Iteration Control

- Mode: `single-pass`

## Result

## Review

## Evidence And Residual Risks
""")
        self.write(READER, "export const layout = 1;\n")
        self.write(DOC, "# Notes\n")
        self.write(".mise.toml", '[tools]\nnode = "24.1.0"\n')
        self.commit("fixture base")
        self.git("branch", "fixture-base")

    def command(self, *args):
        return subprocess.run(
            args, cwd=self.root, env=self.env, capture_output=True, encoding="utf-8"
        )

    def git(self, *args):
        result = self.command("git", *args)
        self.assertEqual(result.returncode, 0, result.stderr or result.stdout)
        return result.stdout

    def write(self, path, contents):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(contents, encoding="utf-8")

    def commit(self, message):
        self.git("add", "-A")
        self.git("commit", "-qm", message)

    def expect(self, command, code, message):
        result = self.command(*command)
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, code, output)
        self.assertIn(message, output)
        return output

    def guard(self, *args, code=0, message):
        return self.expect((sys.executable, "scripts/doc_guard.py", *args), code, message)

    def test_docs_only_workflow_passes_without_building(self):
        self.write(DOC, "# Revised notes\n")
        self.write(ACTIVE, "# Active\n\n- See open issues.\n")
        self.guard(message="ok (docs-only)")
        self.git("switch", "-qc", "candidate")
        self.commit("pure docs")
        self.git("switch", "-qc", "target", "fixture-base")
        self.write(READER, "export const layout = 2;\n")
        self.commit("target-only reader change")
        self.git("switch", "-q", "candidate")
        self.write("artifacts/local/ignored.log", "local evidence\n")
        output = self.expect(
            ("bash", "scripts/check-workflow.sh", "--base", "target"),
            0,
            "ok (docs-only)",
        )
        self.assertIn("doc_length_check: ok", output)
        self.assertIn("doc_guard_self_check: ok", output)

    def test_toolchain_changes_need_an_updated_change(self):
        self.write(".mise.toml", '[tools]\nnode = "24.2.0"\n')
        self.guard(code=1, message="without an updated accepted change")
        self.commit("update toolchain")
        self.guard(
            "--base", "fixture-base", "--kind", "change",
            code=1, message="without an updated accepted change",
        )
        self.write(CHANGE, (self.root / CHANGE).read_text(encoding="utf-8") + "\nUpdate toolchain.\n")
        self.commit("update change record")
        self.guard("--base", "fixture-base", "--kind", "change", message="ok (change)")

    def test_reader_changes_use_the_same_policy_in_both_modes(self):
        self.write(READER, "export const layout = 2;\n")
        self.guard(code=1, message="without an updated accepted change")
        self.commit("update reader")
        self.guard("--base", "fixture-base", code=1, message="without an updated accepted change")
        self.guard("--base", "fixture-base", "--kind", "fast", message="ok (fast)")

    def test_fast_still_validates_active_routes(self):
        missing = "- `change`：Missing -> `docs/changes/missing.md`\n"
        self.write(ACTIVE, "# Active\n" + ROUTE + missing)
        self.guard(code=1, message="docs/ACTIVE.md:3")
        self.commit("malformed route")
        self.guard("--base", "fixture-base", "--kind", "fast", code=1, message="docs/ACTIVE.md:3")
        self.write(ACTIVE, "# Active\n" + ROUTE + missing.replace("->", "→"))
        self.guard(code=1, message="docs/changes/missing.md")

    def test_dirty_candidate_rejects_unstaged_staged_and_untracked_files(self):
        self.write(DOC, "# Revised notes\n")
        self.guard("--base", "fixture-base", code=2, message="needs a clean worktree")
        self.git("add", DOC)
        self.guard("--base", "fixture-base", code=2, message="needs a clean worktree")
        self.commit("pure docs")
        self.write("docs/untracked.md", "untracked\n")
        self.guard("--base", "fixture-base", code=2, message="needs a clean worktree")

    def test_special_filename_is_one_path_in_both_modes(self):
        path = "reader/spaced 名称 -> not a rename\n.ts"
        self.write(path, "export const fixture = true;\n")
        self.guard(code=1, message=path)
        self.commit("add an unusually named reader source")
        self.guard("--base", "fixture-base", code=1, message=path)

    def assert_rename_requires_change(self, source, destination):
        self.git("mv", source, destination)
        output = self.guard(code=1, message="without an updated accepted change")
        self.assertIn(source, output)
        self.assertIn(destination, output)
        self.commit("move across the docs and reader boundary")
        output = self.guard("--base", "fixture-base", code=1, message="without an updated accepted change")
        self.assertIn(source, output)
        self.assertIn(destination, output)

    def test_rename_out_of_reader(self):
        self.assert_rename_requires_change(READER, "docs/moved.ts")

    def test_rename_into_reader(self):
        self.assert_rename_requires_change(DOC, "reader/app/moved.md")

    def test_committed_whitespace_uses_pr_merge_base_in_workflow_and_fast(self):
        self.git("switch", "-qc", "candidate")
        self.write(DOC, "# Candidate with trailing space \n")
        self.commit("candidate")
        self.git("switch", "-qc", "target", "fixture-base")
        self.write(DOC, "# Candidate with trailing space \n")
        self.commit("target advanced independently")
        self.git("switch", "-q", "candidate")
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.git("diff", "--check")
        self.git("diff", "--check", "target", "HEAD")
        for script in ("check-workflow.sh", "check-fast.sh"):
            with self.subTest(script=script):
                output = self.expect(
                    ("bash", f"scripts/{script}", "--base", "target"),
                    1, "docs/notes.md:1: trailing whitespace.",
                )
                self.assertNotIn("Unexpected toolchain command", output)

    def test_fast_without_base_still_checks_worktree_whitespace(self):
        self.write(DOC, "# Worktree with trailing space \n")
        self.expect(("bash", "scripts/check-fast.sh"), 2, "docs/notes.md:1: trailing whitespace.")


if __name__ == "__main__":
    unittest.main()

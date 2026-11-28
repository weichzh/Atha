import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CLEANUP = Path(__file__).resolve().parents[1] / "repo-cleanup.py"


class CleanupTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name) / "repo"
        bare = Path(temp.name) / "remote.git"
        self.env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
        self.root.mkdir()
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Cleanup Test")
        self.git("config", "user.email", "cleanup@example.invalid")
        self.git("config", "commit.gpgSign", "false")
        self.git("commit", "--allow-empty", "-m", "main")
        self.git("init", "--bare", str(bare))
        self.git("remote", "add", "origin", str(bare))
        self.git("push", "-u", "origin", "main")
        self.git("branch", "fix/merged")
        self.git("branch", "docs/held")
        self.git("config", "branch.docs/held.deleteMerged", "false")
        self.git("branch", "docs/checked-out")
        self.git("worktree", "add", str(self.root.parent / "checked-out"), "docs/checked-out")
        self.git("update-ref", "refs/remotes/origin/fix/expired", "HEAD")
        self.git("update-ref", "refs/remotes/origin/docs/used", "HEAD")
        self.git("switch", "-c", "feat/unmerged")
        self.git("commit", "--allow-empty", "-m", "not merged")
        self.git("update-ref", "refs/remotes/origin/docs/expired-unmerged", "HEAD")
        self.git("branch", "feat/depends")
        self.git("config", "branch.feat/depends.remote", "origin")
        self.git("config", "branch.feat/depends.merge", "refs/heads/docs/used")
        self.git("switch", "main")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, env=self.env,
                              capture_output=True, text=True, check=True).stdout.strip()

    def run_cleanup(self, *args, selection=""):
        return subprocess.run([sys.executable, str(CLEANUP), *args], cwd=self.root, env=self.env,
                              input=selection, capture_output=True, text=True)

    def refs(self):
        return self.git("for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes")

    def test_preview_lists_candidates_and_protected_refs_without_mutations(self):
        before = self.refs()
        plan = self.run_cleanup()
        self.assertEqual(plan.returncode, 0, plan.stderr)
        for label in ("fix/merged", "origin/fix/expired", "工作树仍检出", "尚未进入主线",
                      "跟踪引用的提交尚未进入主线", "feat/depends 仍在跟踪", "已配置不自动删除"):
            self.assertIn(label, plan.stdout)
        self.assertEqual(self.refs(), before)

    def test_cancel_and_invalid_selection_leave_refs_alone(self):
        before = self.refs()
        self.assertEqual(self.run_cleanup("--apply", selection="\n").returncode, 0)
        self.assertNotEqual(self.run_cleanup("--apply", selection="1 999\n").returncode, 0)
        self.assertEqual(self.refs(), before)

    def test_selection_only_deletes_numbered_local_refs(self):
        plan = self.run_cleanup().stdout
        numbers = {parts[1]: parts[0] for line in plan.splitlines()
                   if (parts := line.split()) and parts[0].isdigit()}
        self.assertEqual(set(numbers), {"fix/merged", "origin/fix/expired"})
        remote = self.git("ls-remote", "--heads", "origin")
        trees = self.git("worktree", "list", "--porcelain")
        applied = self.run_cleanup("--apply", selection=" ".join(numbers.values()) + "\n")
        self.assertEqual(applied.returncode, 0, applied.stderr)
        self.assertNotIn("refs/heads/fix/merged", self.refs())
        self.assertNotIn("refs/remotes/origin/fix/expired", self.refs())
        self.assertIn("refs/remotes/origin/docs/used", self.refs())
        self.assertIn("refs/remotes/origin/docs/expired-unmerged", self.refs())
        self.assertEqual(self.git("ls-remote", "--heads", "origin"), remote)
        self.assertEqual(self.git("worktree", "list", "--porcelain"), trees)

    def test_changed_branch_is_rejected_after_prompt(self):
        args = [sys.executable, str(CLEANUP), "--apply"]
        with subprocess.Popen(args, cwd=self.root, env=self.env, stdin=subprocess.PIPE,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as proc:
            prompt = ""
            while not prompt.endswith("）："):
                char = proc.stdout.read(1)
                if not char:
                    self.fail(f"未到确认提示：{proc.stderr.read()}")
                prompt += char
            self.git("branch", "-f", "fix/merged", "feat/unmerged")
            _, stderr = proc.communicate("1\n", timeout=10)
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("候选状态变化", stderr)
        self.assertIn("refs/heads/fix/merged", self.refs())

    def test_stale_origin_main_is_rejected(self):
        self.git("push", "origin", "feat/unmerged:refs/heads/main")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        before = self.refs()
        result = self.run_cleanup("--apply", selection="1\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("先运行 git fetch origin main", result.stderr)
        self.assertEqual(self.refs(), before)


if __name__ == "__main__":
    unittest.main()

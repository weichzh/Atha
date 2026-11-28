#!/usr/bin/env python3
"""Preview and select merged short branches and stale origin tracking refs."""
import argparse
import json
import subprocess
import sys
from dataclasses import dataclass


MAIN = "refs/heads/main"
ORIGIN = "refs/remotes/origin/"
SHORT_PREFIXES = ("fix/", "feat/", "docs/")


def git(*args, allow_missing=False):
    result = subprocess.run(["git", *args], capture_output=True, text=True, errors="surrogateescape")
    if result.returncode and not (allow_missing and result.returncode == 1):
        raise RuntimeError(result.stderr.strip() or f"git {args[0]} failed ({result.returncode})")
    return result


@dataclass(frozen=True)
class Entry:
    ref: str
    oid: str
    reason: str = ""


def worktrees():
    fields = git("worktree", "list", "--porcelain", "-z").stdout
    return [dict(field.partition(" ")[::2] for field in row.split("\0") if field)
            for row in fields.split("\0\0") if row.strip("\0")]


def inventory():
    remote_heads = {name: oid for oid, name in
                    (line.split("\t") for line in git("ls-remote", "--heads", "origin").stdout.split("\n") if line)}
    local_origin_main = git("rev-parse", "--verify", f"{ORIGIN}main").stdout.strip()
    if remote_heads.get(MAIN) != local_origin_main:
        raise RuntimeError("本地 origin/main 与远端不一致；先运行 git fetch origin main，再重新预览。")

    fmt = "%(refname)%00%(objectname)%00%(upstream)%00%(symref)"
    refs = [line.split("\0") for line in
            git("for-each-ref", f"--format={fmt}", "refs/heads", ORIGIN).stdout.split("\n") if line]
    merged = set(git("for-each-ref", f"--merged={local_origin_main}", "--format=%(refname)",
                     "refs/heads", ORIGIN).stdout.split("\n"))
    trees = worktrees()
    checked_out = {tree["branch"] for tree in trees if "branch" in tree}
    local_refs = [row for row in refs if row[0].startswith("refs/heads/")]

    def tracked(*targets):
        return next((ref.removeprefix("refs/heads/") for ref, _, upstream, _ in local_refs
                     if ref not in targets and upstream in targets), None)

    entries = []
    for ref, oid, _, symbolic in refs:
        if symbolic:
            continue
        if ref.startswith("refs/heads/"):
            name = ref.removeprefix("refs/heads/")
            if not name.startswith(SHORT_PREFIXES):
                reason = "保留：不是短期分支"
            elif ref in checked_out:
                reason = "保留：工作树仍检出"
            elif ref not in merged:
                reason = "保留：提交尚未进入主线"
            elif git("config", "--type=bool", "--get", f"branch.{name}.deleteMerged",
                     allow_missing=True).stdout.strip() == "false":
                reason = "保留：分支已配置不自动删除"
            elif dependent := tracked(ref, f"{ORIGIN}{name}"):
                reason = f"保留：{dependent} 仍在跟踪"
            else:
                reason = ""
        else:
            remote_branch = "refs/heads/" + ref.removeprefix(ORIGIN)
            if remote_branch in remote_heads:
                continue
            if ref not in merged:
                reason = "保留：跟踪引用的提交尚未进入主线"
            elif dependent := tracked(ref):
                reason = f"保留：{dependent} 仍在跟踪"
            else:
                reason = ""
        entries.append(Entry(ref, oid, reason))
    return local_origin_main, entries, trees


def show(entries, trees):
    print("工作树（仅展示，不删除）：")
    for tree in trees:
        name = tree.get("branch", "").removeprefix("refs/heads/") or "detached"
        flags = "；可回收状态" if "prunable" in tree else "；已锁定" if "locked" in tree else ""
        print(f"  {json.dumps(tree['worktree'], ensure_ascii=False)} [{name}{flags}]")
    choices = []
    print("本地分支与过期的 origin 跟踪引用：")
    for entry in entries:
        name = entry.ref.removeprefix("refs/heads/").removeprefix("refs/remotes/")
        if entry.reason:
            label = " —"
        else:
            choices.append(entry)
            label = f"{len(choices):2d}"
        print(f"  {label}  {name}  {entry.oid[:8]}  {entry.reason or '可选清理'}")
    if not choices:
        print("没有可清理的引用。")
    return choices


def on_synced_main(tip):
    return (git("symbolic-ref", "--quiet", "HEAD", allow_missing=True).stdout.strip() == MAIN
            and git("rev-parse", "HEAD").stdout.strip() == tip)


def main():
    parser = argparse.ArgumentParser(description="预览本地分支清理，不删除工作树、文件或远端分支。")
    parser.add_argument("--apply", action="store_true", help="显示清单后输入要删除的编号（空行取消）")
    args = parser.parse_args()

    tip, entries, trees = inventory()
    choices = show(entries, trees)
    if not args.apply or not choices:
        return
    if not on_synced_main(tip):
        raise RuntimeError("执行清理前，请在与 origin/main 同步的本地 main 工作目录运行。")
    try:
        selection = input("输入编号（空格或逗号分隔；空行取消）：").replace(",", " ").split()
    except EOFError:
        selection = []
    if not selection:
        print("已取消，没有删除引用。")
        return
    if not all(value.isascii() and value.isdecimal() and 1 <= int(value) <= len(choices)
               for value in selection):
        raise RuntimeError("编号无效，没有删除引用。")
    picked = [choices[i - 1] for i in dict.fromkeys(map(int, selection))]

    new_tip, fresh, _ = inventory()
    eligible = {entry for entry in fresh if not entry.reason}
    if tip != new_tip or not on_synced_main(new_tip) or not set(picked) <= eligible:
        raise RuntimeError("主线或候选状态变化，没有删除引用；请重新预览。")

    for entry in picked:
        if entry.ref.startswith(ORIGIN):
            git("update-ref", "--no-deref", "-d", entry.ref, entry.oid)
            print(f"已删除跟踪引用：{entry.ref.removeprefix('refs/remotes/')}")
        else:
            name = entry.ref.removeprefix("refs/heads/")
            if git("rev-parse", "--verify", entry.ref).stdout.strip() != entry.oid:
                raise RuntimeError(f"{name} 已变化，停止清理其余引用。")
            print(git("branch", "-d", "--", name).stdout.strip())


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, KeyboardInterrupt) as error:
        print(f"清理停止：{error or '操作者中断'}", file=sys.stderr)
        sys.exit(1)

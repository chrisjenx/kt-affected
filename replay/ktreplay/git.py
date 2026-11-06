"""Thin git helpers over a blobless clone."""

import os
import subprocess


def run(repo_dir, *args, check=True, input=None):
    return subprocess.run(["git", "-C", repo_dir, *args], capture_output=True, text=True,
                          check=check, input=input)


def out(repo_dir, *args):
    return run(repo_dir, *args).stdout


def ensure_clone(url, dest):
    """A blobless clone of `url` at `dest`, fetched up to date (blobs come on demand)."""
    if not os.path.isdir(os.path.join(dest, ".git")) and not os.path.exists(os.path.join(dest, "HEAD")):
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        subprocess.run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout", url, dest], check=True)
    run(dest, "fetch", "-q", "--filter=blob:none", "origin", "+refs/heads/*:refs/remotes/origin/*")
    return dest


def has_commit(repo_dir, sha):
    return run(repo_dir, "cat-file", "-e", f"{sha}^{{commit}}", check=False).returncode == 0


def fetch_commit(repo_dir, sha):
    """Fetches one commit by id (PR heads that are on no branch); False when the server refuses."""
    if has_commit(repo_dir, sha):
        return True
    return run(repo_dir, "fetch", "-q", "--filter=blob:none", "origin", sha, check=False).returncode == 0


def changed_files(repo_dir, base, head):
    return [l for l in out(repo_dir, "diff", "--name-only", "--no-renames", base, head).splitlines() if l]


def ls_tree(repo_dir, sha):
    return out(repo_dir, "ls-tree", "-r", "--name-only", sha).splitlines()


def ensure_worktree(repo_dir, path, sha):
    """A detached work tree of `repo_dir` at `path`, sharing its objects."""
    if not os.path.exists(os.path.join(path, ".git")):
        run(repo_dir, "worktree", "prune")
        run(repo_dir, "worktree", "add", "-q", "-f", "--detach", path, sha)
    return path

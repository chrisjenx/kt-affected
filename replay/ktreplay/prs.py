"""Merged pull requests and the commit range each one landed as."""

import json
import subprocess

from . import git

FIELDS = (
    "number,title,author,createdAt,mergedAt,mergeCommit,baseRefName,headRefName,"
    "headRepositoryOwner,headRefOid"
)


def merged_prs(gh, since, until, limit):
    """Merged PRs with `since <= mergedAt <= until` (ISO dates), newest first, at most `limit`."""

    def produce():
        p = subprocess.run(
            ["gh", "pr", "list", "-R", gh.repo, "--state", "merged", "--limit", str(limit),
             "--search", f"merged:{since}..{until} sort:updated-desc", "--json", FIELDS],
            capture_output=True, text=True, check=True,
        )
        return json.loads(p.stdout)

    rows = gh.cached_json(f"prs:{since}:{until}:{limit}", None if until < today() else 6 * 3600, produce)
    out = []
    for r in rows:
        if not r.get("mergeCommit"):
            continue
        out.append({
            "number": r["number"],
            "title": r["title"],
            "author": (r.get("author") or {}).get("login"),
            "createdAt": r["createdAt"],
            "mergedAt": r["mergedAt"],
            "merge": r["mergeCommit"]["oid"],
            "baseRef": r["baseRefName"],
            "headRef": r["headRefName"],
            "headOwner": (r.get("headRepositoryOwner") or {}).get("login"),
            "headOid": r["headRefOid"],
        })
    out.sort(key=lambda r: r["mergedAt"])
    return out


def today():
    import datetime
    return datetime.date.today().isoformat()


def resolve_range(gh, repo_dir, pr):
    """Sets `pr["base"]`, `pr["head"]` and `pr["mergeKind"]`.

    merge:  the merge commit has two parents; base = first parent, head = merge commit.
    squash: one parent and the PR's commits are not replayed on the base branch; base = parent.
    rebase: the last n first-parent commits carry the PR's n commit subjects; base = merge~n.
    """
    m = pr["merge"]
    parents = git.out(repo_dir, "rev-list", "--parents", "-n1", m).split()[1:]
    if len(parents) >= 2:
        pr.update(base=parents[0], head=m, mergeKind="merge")
        return pr
    # Only a one-parent landing needs the PR's own commits (REST, cached): squash or rebase?
    commits = gh.paged(f"repos/{gh.repo}/pulls/{pr['number']}/commits", None)
    headlines = [c["commit"]["message"].split("\n", 1)[0] for c in commits]
    n = len(commits)
    if n > 1:
        subjects = git.out(repo_dir, "log", "--first-parent", f"-n{n}", "--format=%s", m).splitlines()
        if list(reversed(subjects)) == headlines:
            base = git.out(repo_dir, "rev-parse", f"{m}~{n}").strip()
            pr.update(base=base, head=m, mergeKind="rebase")
            return pr
    pr.update(base=parents[0], head=m, mergeKind="squash" if n > 1 else "squash-or-rebase-1")
    return pr

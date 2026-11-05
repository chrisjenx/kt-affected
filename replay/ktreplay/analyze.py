"""Failure rows per merged PR, their attribution, and each strategy's recall and cost."""

import datetime
import re

from .oracle import SOURCE_RANK, is_test_task

QUEUE_BRANCH = re.compile(r"gh-readonly-queue/[^/]+/pr-(\d+)-")
STRATEGIES = ["kt-affected", "kt-affected+assist", "module-level", "run-everything"]


def iso(ts):
    return datetime.datetime.fromisoformat(ts.replace("Z", "+00:00"))


def run_identity(run, prs_by_number, prs_by_branch):
    """Which pull request (or base-branch push) a run belongs to: `pr:<n>`, `push:<branch>`, or
    `branch:<repo>:<branch>` for a pull request outside the replayed set."""
    if run["event"] == "push":
        return f"push:{run['headBranch']}"
    if run["event"] == "merge_group":
        m = QUEUE_BRANCH.search(run["headBranch"] or "")
        return f"pr:{m.group(1)}" if m else f"queue:{run['headBranch']}"
    for n in run["prs"]:
        return f"pr:{n}"
    owner = (run["headRepo"] or "").split("/")[0]
    t = iso(run["createdAt"])
    for pr in prs_by_branch.get((owner, run["headBranch"]), []):
        if iso(pr["createdAt"]) - datetime.timedelta(days=1) <= t <= iso(pr["mergedAt"]) + datetime.timedelta(hours=1):
            return f"pr:{pr['number']}"
    return f"branch:{run['headRepo']}:{run['headBranch']}"


def module_of_dir(discovery, moddir):
    for p in discovery.get("projects", []):
        if p["dir"].rstrip("/") == moddir:
            return p["path"]
    return ":" + moddir.replace("/", ":") if moddir else ":"


def split_task(path):
    mod, _, task = path.rpartition(":")
    return (mod or ":"), task


def normalize_class(cls, classes):
    """The inventory class a reported class belongs to (nested, parameterised and `$` names map to
    their top-level class), or the reported name when none matches."""
    c = re.sub(r"\[.*$", "", cls)
    if c in classes:
        return c, True
    for cand in (c.split("$")[0],):
        if cand in classes:
            return cand, True
    parts = c.split(".")
    for i in range(len(parts) - 1, 0, -1):
        cand = ".".join(parts[:i])
        if cand in classes:
            return cand, True
    simple = [k for k in classes if k.rsplit(".", 1)[-1] == c]
    if len(simple) == 1:
        return simple[0], True
    return c, False


def inventory_index(inventory):
    """{(module, task): set(classes)} and {class: {(module, task)}}."""
    by_group = {}
    by_class = {}
    for g in inventory:
        k = (g["module"], g["task"])
        by_group.setdefault(k, set()).update(g["classes"])
        for c in g["classes"]:
            by_class.setdefault(c, set()).add(k)
    return by_group, by_class


def resolve_events(events, identity_of_run, sel_by_pr, fallback_sel):
    """Failure occurrences: [{identity, module, task, cls, resolved, source, runId, attempt, ...}]."""
    out = []
    for e in events:
        ident = identity_of_run[e["runId"]]
        pr_n = int(ident[3:]) if ident.startswith("pr:") else None
        sel = sel_by_pr.get(pr_n) or fallback_sel
        inv = sel["inventory"]
        by_group, by_class = inventory_index(inv)
        all_classes = set(by_class)
        if e.get("moduleDir") is not None:
            if not is_test_task(e["taskName"]):
                continue
            module, task = module_of_dir(sel["discovery"], e["moduleDir"]), e["taskName"]
        elif e["task"]:
            module, task = split_task(e["task"])
        else:
            module = task = None
        cls = e["cls"]
        resolved = None
        if cls and task and cls.startswith(task + "."):
            cls = cls[len(task) + 1:]  # Kotlin/JS and Wasm reports prefix the class with the task
        if cls:
            scope = by_group.get((module, task), set()) if module else all_classes
            cls, resolved = normalize_class(cls, scope or all_classes)
            if module is None:
                # --quiet logs: pick the failed test task whose inventory holds the class.
                cands = [split_task(t) for t in e.get("failedTasks", [])]
                hits = [k for k in cands if cls in by_group.get(k, ())]
                if len(hits) == 1:
                    module, task = hits[0]
                elif len(cands) == 1:
                    module, task = cands[0]
                elif cls in by_class and len(by_class[cls]) == 1:
                    module, task = next(iter(by_class[cls]))
        out.append(dict(e, identity=ident, module=module, task=task, cls=cls, resolved=resolved))
    return out


def rows_for_prs(occ, prs):
    """One row per (PR, module, task, class) failure, keeping every occurrence's provenance, and
    task-only rows for failing test tasks no source named a class for."""
    by_pr = {}
    for o in occ:
        if not o["identity"].startswith("pr:"):
            continue
        n = int(o["identity"][3:])
        if n not in prs:
            continue
        by_pr.setdefault(n, []).append(o)
    rows = []
    for n, items in by_pr.items():
        classed = {}
        task_only = {}
        named_tasks = set()
        for o in items:
            if o["cls"]:
                k = (o["module"], o["task"], o["cls"])
                classed.setdefault(k, []).append(o)
                named_tasks.add((o["runId"], o.get("attempt"), o["module"], o["task"]))
        for o in items:
            if not o["cls"] and o["module"]:
                runs_with_classes = {(r, m, t) for (r, a, m, t) in named_tasks}
                if (o["runId"], o["module"], o["task"]) in runs_with_classes:
                    continue
                task_only.setdefault((o["module"], o["task"]), []).append(o)
        for (module, task, cls), occs in sorted(classed.items(), key=lambda kv: tuple(map(str, kv[0]))):
            rows.append(make_row(n, module, task, cls, occs))
        for (module, task), occs in sorted(task_only.items()):
            r = make_row(n, module, task, None, occs)
            r["taskOnly"] = True
            rows.append(r)
    return rows


def make_row(n, module, task, cls, occs):
    sources = sorted({o["source"] for o in occs}, key=SOURCE_RANK.get)
    return {
        "pr": n, "module": module, "task": task, "class": cls,
        "resolved": all(o.get("resolved", True) is not False for o in occs),
        "sources": sources,
        "occurrences": [{"runId": o["runId"], "attempt": o.get("attempt"), "job": o.get("job"),
                         "headSha": o["headSha"], "createdAt": o["createdAt"], "source": o["source"],
                         "method": o.get("method")} for o in occs],
        "firstFailure": min(o["createdAt"] for o in occs),
    }


def flake_evidence(row, occ, base_branches, window_days):
    """Other pull requests and base-branch pushes where the same class failed within the window."""
    if not row["class"]:
        return {"otherPrs": [], "basePushes": []}
    t0 = iso(row["firstFailure"])
    w = datetime.timedelta(days=window_days)
    me = f"pr:{row['pr']}"
    others, pushes = set(), set()
    for o in occ:
        if o["cls"] != row["class"] or o["identity"] == me:
            continue
        if abs(iso(o["createdAt"]) - t0) > w:
            continue
        if o["identity"].startswith("push:"):
            if o["identity"][5:] in base_branches:
                pushes.add(o["runId"])
        else:
            others.add(o["identity"])
    return {"otherPrs": sorted(others), "basePushes": sorted(pushes)}


def file_of_class(tree, cls):
    """Paths in the tree that can declare top-level class `cls` (by file name and package dirs)."""
    simple = cls.rsplit(".", 1)[-1]
    pkg = cls.rsplit(".", 1)[0].replace(".", "/") if "." in cls else ""
    hits = [p for p in tree if p.endswith(f"/{simple}.kt") or p.endswith(f"/{simple}.java")]
    exact = [p for p in hits if pkg and f"/{pkg}/" in p]
    return exact or hits


def selected(groups, module, task, cls):
    """'class' when the group runs the class, 'task' when the whole task runs, None otherwise."""
    for g in groups:
        if g["module"] == module and g["task"] == task:
            if cls is None:
                return "task"
            if g["full"]:
                return "task"
            if cls in g["classes"]:
                return "class"
            return None
    return None


def build_logic_only(changed):
    return changed and not any(f.endswith((".kt", ".java", ".kts")) and "/src/" in f for f in changed)

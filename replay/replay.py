#!/usr/bin/env python3
"""Public accuracy replay: would kt-affected have run the tests that really failed in CI because of
a pull request's diff? See docs/replay.md.

  python3 replay/replay.py run <target> [--days 90] [--until YYYY-MM-DD] [--limit 1000]
  python3 replay/replay.py report <target> [same options]     # re-analyse cached data only
  python3 replay/replay.py docs                                # refresh docs/replay.md's results table

Needs Python 3.11+, git, an authenticated `gh` and a Rust toolchain (it builds kt-affected and the
`replay` example). Everything fetched is cached under `.replay/`, so an interrupted run resumes.
"""

import argparse
import datetime
import glob
import gzip
import json
import os
import re
import statistics
import subprocess
import sys
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ktreplay import analyze, git, oracle, prs as prmod, report, select  # noqa: E402
from ktreplay.gh import GH  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HERE = os.path.join(ROOT, "replay")
STATE = os.path.join(ROOT, ".replay")


def log(*a):
    print(*a, file=sys.stderr, flush=True)


def load_targets():
    with open(os.path.join(HERE, "targets.toml"), "rb") as fh:
        return tomllib.load(fh)


def build_binaries():
    subprocess.run(["cargo", "build", "-q", "--release", "--bin", "kt-affected", "--example", "replay"],
                   cwd=ROOT, check=True)
    rel = os.path.join(ROOT, "target", "release")
    return os.path.join(rel, "kt-affected"), os.path.join(rel, "examples", "replay")


def kta_version():
    p = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short=10", "HEAD"], capture_output=True, text=True)
    with open(os.path.join(ROOT, "Cargo.toml")) as fh:
        version = next(l.split('"')[1] for l in fh if l.startswith("version"))
    return f"{version} ({p.stdout.strip()})"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report", "docs"])
    ap.add_argument("target", nargs="?")
    ap.add_argument("--days", type=int, default=90)
    ap.add_argument("--until", default=datetime.date.today().isoformat())
    ap.add_argument("--limit", type=int, default=1000, help="at most this many merged PRs")
    ap.add_argument("--flake-days", type=int, default=7)
    ap.add_argument("--workers", type=int, default=1, help="PRs selected in parallel, one work tree each")
    ap.add_argument("--out-date", default=None, help="results directory date (default: --until)")
    args = ap.parse_args()
    if args.cmd == "docs":
        report.refresh_docs(ROOT)
        return
    targets = load_targets()
    if args.target not in targets:
        sys.exit(f"unknown target {args.target!r}; known: {', '.join(targets)}")
    cfg = targets[args.target]
    run(args, args.target, cfg, offline=args.cmd == "report")


def run(args, name, cfg, offline):
    cache = os.path.join(STATE, "cache", name)
    paths = {
        "repo": os.path.join(STATE, "repos", name),
        "select": os.path.join(cache, "select"),
        "kta": os.path.join(cache, "kta"),
    }
    gh = GH(cfg["repo"], cache, offline=offline)
    until = args.until
    since = (datetime.date.fromisoformat(until) - datetime.timedelta(days=args.days)).isoformat()
    runs_since = (datetime.date.fromisoformat(since) - datetime.timedelta(days=14)).isoformat()

    paths["cli"], paths["replay_bin"] = build_binaries()
    if not offline:
        git.ensure_clone(f"https://github.com/{cfg['repo']}.git", paths["repo"])

    log(f"[{name}] merged PRs {since}..{until}")
    prs = prmod.merged_prs(gh, since, until, args.limit)
    for pr in prs:
        if not git.has_commit(paths["repo"], pr["merge"]):
            git.fetch_commit(paths["repo"], pr["merge"])
        prmod.resolve_range(gh, paths["repo"], pr)
    prs_by_number = {p["number"]: p for p in prs}
    prs_by_branch = {}
    for p in prs:
        prs_by_branch.setdefault((p["headOwner"], p["headRef"]), []).append(p)
    log(f"[{name}] {len(prs)} PRs; listing runs of {', '.join(cfg['workflows'])} since {runs_since}")

    runs = oracle.list_runs(gh, cfg["workflows"], runs_since, until)
    identity = {r["id"]: analyze.run_identity(r, prs_by_number, prs_by_branch) for r in runs}
    log(f"[{name}] {len(runs)} runs; reading failures")
    events, reports, ostats = oracle.collect(gh, cfg, runs, cache, log=log)
    log(f"[{name}] {len(events)} failure events ({gh.calls} API calls); selecting")

    empty = git.empty_commit(paths["repo"])
    sel = select.select_all(cfg, paths, prs, empty, args.workers, log)

    results = analyse(args, name, cfg, prs, runs, identity, events, reports, sel, paths)
    results["oracleStats"] = ostats
    results["window"] = {"since": since, "until": until, "runsSince": runs_since}
    out_dir = os.path.join(HERE, "results", name, args.out_date or until)
    report.write(out_dir, name, cfg, results)
    log(f"[{name}] wrote {os.path.relpath(out_dir, ROOT)}")


def reference(cfg, res):
    """The selection result whose inventory and discovery the baselines and shares use."""
    return res.get("kt-affected+assist") or res["kt-affected"]


def ci_groups(cfg, groups):
    tasks = set(cfg["ci_tasks"])
    return [g for g in groups if g["task"] in tasks]


def load_review(name):
    f = os.path.join(HERE, "review", f"{name}.json")
    if not os.path.exists(f):
        return {}
    with open(f) as fh:
        items = json.load(fh)
    return {(i["pr"], i["module"], i["task"], i["class"]): i for i in items}


def durations(reports, ref_discovery):
    """Median per-class milliseconds over every parsed test report: {(module, task, class): ms}."""
    acc = {}
    for _, parsed in reports:
        for (moddir, task), classes in parsed.items():
            if not oracle.is_test_task(task):
                continue
            module = analyze.module_of_dir(ref_discovery, moddir)
            for cls, (t, f, sk, ms, kind) in classes.items():
                if ms is not None:
                    acc.setdefault((module, task, cls), []).append(ms)
    return {k: statistics.median(v) for k, v in acc.items()}


def analyse(args, name, cfg, prs, runs, identity, events, reports, sel, paths):
    review = load_review(name)
    numbers = {p["number"] for p in prs}
    pr_by_n = {p["number"]: p for p in prs}
    ref_by_pr = {n: reference(cfg, r) for n, r in sel.items()}
    latest = ref_by_pr[prs[-1]["number"]] if prs else {"inventory": [], "discovery": {}}
    occ = analyze.resolve_events(events, identity, ref_by_pr, latest)
    rows = analyze.rows_for_prs(occ, numbers)
    base_branches = {p["baseRef"] for p in prs}
    ms = durations(reports, latest["discovery"])
    trees = {}

    strategies = [s for s in analyze.STRATEGIES if s != "kt-affected+assist" or cfg.get("include_projects")]

    def strategy_groups(n):
        res = sel[n]
        ref = ref_by_pr[n]
        inv = ci_groups(cfg, ref["inventory"])
        hit, _ = select.module_level(ref["discovery"], res["changedFiles"], inv)
        out = {
            "kt-affected": ci_groups(cfg, res["kt-affected"]["groups"]),
            "module-level": [dict(g, full=True) for g in inv if g["module"] in hit],
            "run-everything": [dict(g, full=True) for g in inv],
        }
        if "kt-affected+assist" in res:
            out["kt-affected+assist"] = ci_groups(cfg, res["kt-affected+assist"]["groups"])
        return out

    groups_by_pr = {n: strategy_groups(n) for n in sel}
    modules_by_pr = {n: select.module_level(ref_by_pr[n]["discovery"], sel[n]["changedFiles"],
                                            ref_by_pr[n]["inventory"]) for n in sel}

    for r in rows:
        n = r["pr"]
        res = sel[n]
        ref = ref_by_pr[n]
        pr = pr_by_n[n]
        if r["class"]:
            if pr["head"] not in trees:
                trees[pr["head"]] = git.ls_tree(paths["repo"], pr["head"])
            r["files"] = analyze.file_of_class(trees[pr["head"]], r["class"])
        else:
            r["files"] = []
        changed = set(res["changedFiles"])
        r["testChanged"] = any(f in changed for f in r["files"])
        r["flake"] = analyze.flake_evidence(r, occ, base_branches, args.flake_days)
        r["inInventory"] = bool(r["class"]) and analyze.selected(
            [dict(g, full=False) for g in ref["inventory"]], r["module"], r["task"], r["class"]) is not None
        r["selected"] = {s: analyze.selected(groups_by_pr[n][s], r["module"], r["task"], r["class"])
                         for s in strategies}
        # The baselines run real Gradle tasks, not kt-affected's predicted ones: module-level runs
        # every test task of an affected module, run-everything every task there is.
        hit, everything = modules_by_pr[n]
        if everything or r["module"] in hit:
            r["selected"]["module-level"] = "task"
        r["selected"]["run-everything"] = "task"
        best = "kt-affected+assist" if "kt-affected+assist" in res else "kt-affected"
        path = res[best]["paths"].get(r["class"]) if r["class"] else None
        r["path"] = path
        r["changedDeclarations"] = [f"{v['file']}: {v['declaration']}" for v in res[best]["verdicts"] if v["functional"]][:40]
        r["nameHits"] = []
        if r["class"] and not r["selected"].get(best) and r["files"]:
            # Name oracle for hand review: changed declarations the test file's text names.
            text = git.run(paths["repo"], "show", f"{pr['head']}:{r['files'][0]}", check=False).stdout
            words = set(re.findall(r"[A-Za-z_]\w*", text))
            r["nameHits"] = sorted({v["declaration"].split(".")[-1] for v in res[best]["verdicts"]
                                    if v["functional"] and v["declaration"].split(".")[-1] in words})
        # Attribution: a hand verdict wins; then the flake rule; then a changed test file; then a
        # graph path from a changed declaration; everything else waits for hand review.
        key = (n, r["module"], r["task"], r["class"])
        fl = r["flake"]
        if key in review:
            r["attribution"] = review[key]["attribution"]
            r["note"] = review[key].get("note", "")
            r["by"] = "hand"
        elif r.get("taskOnly"):
            r["attribution"], r["by"] = "task-only", "auto"
        elif len(fl["otherPrs"]) >= 2 or fl["basePushes"]:
            r["attribution"], r["by"] = "c", "auto-flake"
        elif r["testChanged"]:
            r["attribution"], r["by"] = "a", "auto"
        elif path:
            r["attribution"], r["by"] = "b", "auto"
        else:
            r["attribution"], r["by"] = "review", "pending"

    per_pr = []
    for p in prs:
        n = p["number"]
        res = sel[n]
        ref = ref_by_pr[n]
        inv = ci_groups(cfg, ref["inventory"])
        total = sum(g["total"] for g in inv)
        inv_ms = {(g["module"], g["task"]): g["classes"] for g in inv}
        total_ms = sum(ms.get((m, t, c), 0) for (m, t), cs in inv_ms.items() for c in cs)
        entry = {"pr": n, "mergeKind": p["mergeKind"], "base": p["base"], "head": p["head"],
                 "author": p["author"], "mergedAt": p["mergedAt"], "changedFiles": len(res["changedFiles"]),
                 "totalClasses": total, "rows": sum(1 for r in rows if r["pr"] == n and r["class"]),
                 "errors": {s: res[s]["error"] for s in ("kt-affected", "kt-affected+assist") if s in res and res[s]["error"]},
                 "strategies": {}}
        for s in strategies:
            gs = groups_by_pr[n][s]
            count = 0
            t_ms = 0
            for g in gs:
                cs = inv_ms.get((g["module"], g["task"]), [])
                chosen = cs if g["full"] else g["classes"]
                count += g["total"] if g["full"] else len(g["classes"])
                t_ms += sum(ms.get((g["module"], g["task"], c), 0) for c in chosen)
            entry["strategies"][s] = {
                "classes": count,
                "share": (count / total) if total else None,
                "timeShare": (t_ms / total_ms) if total_ms else None,
            }
        per_pr.append(entry)

    priced_classes = len(ms)
    return {"target": name, "repo": cfg["repo"], "ktAffected": kta_version(), "strategies": strategies,
            "prs": per_pr, "rows": rows,
            "failures": [{k: o.get(k) for k in ("identity", "runId", "attempt", "job", "headSha", "createdAt",
                                                 "module", "task", "cls", "source")} for o in occ], "runs": len(runs), "pricedClasses": priced_classes,
            "generated": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")}


if __name__ == "__main__":
    main()

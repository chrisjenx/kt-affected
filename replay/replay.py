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
    ap.add_argument("cmd", choices=["run", "report", "docs", "check", "compare"])
    ap.add_argument("target", nargs="?")
    ap.add_argument("--days", type=int, default=90)
    ap.add_argument("--until", default=datetime.date.today().isoformat())
    ap.add_argument("--limit", type=int, default=1000, help="at most this many merged PRs")
    ap.add_argument("--flake-days", type=int, default=7)
    ap.add_argument("--workers", type=int, default=1, help="PRs selected in parallel, one work tree each")
    ap.add_argument("--out-date", default=None, help="results directory date (default: --until)")
    ap.add_argument("--freeze", action="store_true", help="write the window's PRs and CI evidence to replay/frozen/")
    ap.add_argument("--frozen", action="store_true", help="replay the committed window in replay/frozen/: no GitHub API")
    ap.add_argument("--shard", help="I/N: select only every N-th PR from the I-th, and write the selections")
    ap.add_argument("--shard-out", help="directory for --shard output (default .replay/shards)")
    ap.add_argument("--shards-from", help="score the window from the shard files in this directory")
    ap.add_argument("--update", action="store_true", help="check: rewrite replay/check/<target>.json")
    args = ap.parse_args()
    if args.cmd == "docs":
        report.refresh_docs(ROOT)
        return
    if args.cmd == "compare":
        compare(args.target, args.out_date or "ci")
        return
    targets = load_targets()
    if args.target not in targets:
        sys.exit(f"unknown target {args.target!r}; known: {', '.join(targets)}")
    cfg = targets[args.target]
    if args.cmd == "check":
        check(args, args.target, cfg)
        return
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

    frozen_file = os.path.join(HERE, "frozen", f"{name}.json.gz")
    if args.frozen:
        # The committed window: PRs, runs and failure evidence as fetched, no GitHub API calls.
        with gzip.open(frozen_file, "rt") as fh:
            frozen = json.load(fh)
        since, until, runs_since = frozen["window"]["since"], frozen["window"]["until"], frozen["window"]["runsSince"]
        prs = frozen["prs"]
        for pr in prs:
            for sha in (pr["base"], pr["head"]):
                if not git.has_commit(paths["repo"], sha):
                    git.fetch_commit(paths["repo"], sha)
    else:
        log(f"[{name}] merged PRs {since}..{until}")
        prs = prmod.merged_prs(gh, since, until, args.limit)
        for pr in prs:
            if not git.has_commit(paths["repo"], pr["merge"]):
                git.fetch_commit(paths["repo"], pr["merge"])
            prmod.resolve_range(gh, paths["repo"], pr)
    if args.shard:
        i, n = (int(x) for x in args.shard.split("/"))
        mine = prs[i - 1::n]
    prs_by_number = {p["number"]: p for p in prs}
    prs_by_branch = {}
    for p in prs:
        prs_by_branch.setdefault((p["headOwner"], p["headRef"]), []).append(p)
    log(f"[{name}] {len(prs)} PRs; listing runs of {', '.join(cfg['workflows'])} since {runs_since}")

    if args.frozen:
        runs, events, ostats = frozen["runs"], frozen["events"], frozen["oracleStats"]
        reports = [(rid, {(d, t): c for d, t, c in parsed}) for rid, parsed in frozen["reports"]]
    else:
        runs = oracle.list_runs(gh, cfg["workflows"], runs_since, until)
        log(f"[{name}] {len(runs)} runs; reading failures")
        events, reports, ostats = oracle.collect(gh, cfg, runs, cache, log=log)
        log(f"[{name}] {len(events)} failure events ({gh.calls} API calls)")
    if args.freeze:
        os.makedirs(os.path.dirname(frozen_file), exist_ok=True)
        with gzip.open(frozen_file, "wt", compresslevel=9) as fh:
            json.dump({"window": {"since": since, "until": until, "runsSince": runs_since}, "prs": prs,
                       "runs": runs, "events": events, "oracleStats": ostats,
                       "reports": [(rid, [[d, t, c] for (d, t), c in parsed.items()]) for rid, parsed in reports]},
                      fh, sort_keys=True, separators=(",", ":"))
        log(f"[{name}] froze {os.path.relpath(frozen_file, ROOT)}")
    identity = {r["id"]: analyze.run_identity(r, prs_by_number, prs_by_branch) for r in runs}

    if args.shard:
        # One shard's selections only; `replay.py merge` scores the whole window from all shards.
        sel = select.select_all(cfg, paths, mine, args.workers, log)
        out = os.path.join(args.shard_out or os.path.join(STATE, "shards"), f"{name}-{args.shard.replace('/', 'of')}.json.gz")
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with gzip.open(out, "wt") as fh:
            json.dump({str(k): v for k, v in sel.items()}, fh)
        log(f"[{name}] shard {args.shard}: {len(mine)} PRs -> {out}")
        return
    if args.shards_from:
        sel = {}
        for f in sorted(glob.glob(os.path.join(args.shards_from, f"{name}-*.json.gz"))):
            with gzip.open(f, "rt") as fh:
                sel.update({int(k): v for k, v in json.load(fh).items()})
        missing = [p["number"] for p in prs if p["number"] not in sel]
        if missing:
            sys.exit(f"[{name}] shards miss {len(missing)} PRs, e.g. {missing[:5]}")
    else:
        sel = select.select_all(cfg, paths, prs, args.workers, log)

    results = analyse(args, name, cfg, prs, runs, identity, events, reports, sel, paths)
    results["oracleStats"] = ostats
    results["window"] = {"since": since, "until": until, "runsSince": runs_since}
    out_dir = os.path.join(HERE, "results", name, args.out_date or until)
    report.write(out_dir, name, cfg, results)
    log(f"[{name}] wrote {os.path.relpath(out_dir, ROOT)}")


def check(args, name, cfg):
    """The per-change gate: re-select the committed window's PRs with real failures kt-affected
    catches (none may be lost) and a fixed sample whose selections are committed (any change is a
    diff to review and `--update`). Frozen evidence only: no GitHub API, minutes not hours."""
    cache = os.path.join(STATE, "cache", name)
    paths = {"repo": os.path.join(STATE, "repos", name), "select": os.path.join(cache, "select"),
             "kta": os.path.join(cache, "kta")}
    paths["cli"], paths["replay_bin"] = build_binaries()
    git.ensure_clone(f"https://github.com/{cfg['repo']}.git", paths["repo"])
    with gzip.open(os.path.join(HERE, "frozen", f"{name}.json.gz"), "rt") as fh:
        prs = json.load(fh)["prs"]
    by_n = {p["number"]: p for p in prs}
    expect_file = os.path.join(HERE, "check", f"{name}.json")
    if args.update or not os.path.exists(expect_file):
        latest = sorted(glob.glob(os.path.join(HERE, "results", name, "*", "rows.json")))[-1]
        with open(latest) as fh:
            rows = json.load(fh)
        caught = sorted({(r["pr"], r["module"], r["task"], r["class"]) for r in rows
                         if r["attribution"][:1] in "ab" and r["selected"].get("kt-affected") and r["pr"] in by_n},
                        key=lambda k: (k[0], k[1], k[2], k[3] or ""))
        sample = check_sample(prs, paths["repo"])
    else:
        with open(expect_file) as fh:
            expect = json.load(fh)
        caught = [tuple(f) for f in expect["caught"]]
        sample = [int(n) for n in expect["sample"]]
    needed = sorted({k[0] for k in caught} | set(sample))
    for n in needed:
        for sha in (by_n[n]["base"], by_n[n]["head"]):
            if not git.has_commit(paths["repo"], sha):
                git.fetch_commit(paths["repo"], sha)
    log(f"[{name}] check: {len(caught)} caught failures over {len({k[0] for k in caught})} PRs, {len(sample)} sampled PRs")
    sel = select.select_all(cfg, paths, [by_n[n] for n in needed], args.workers, log)

    def groups(n):
        return ci_groups(cfg, sel[n]["kt-affected"]["groups"])

    def snapshot(n):
        return sorted(f"{g['module']}:{g['task']}:{c}" for g in groups(n)
                      for c in (["*"] if g["full"] else g["classes"]))

    lost = [k for k in caught if not analyze.selected(groups(k[0]), k[1], k[2], k[3])]
    snaps = {str(n): snapshot(n) for n in sample}
    if args.update or not os.path.exists(expect_file):
        os.makedirs(os.path.dirname(expect_file), exist_ok=True)
        with open(expect_file, "w") as fh:
            json.dump({"caught": [list(k) for k in caught], "sample": snaps}, fh, indent=1, sort_keys=True)
            fh.write("\n")
        log(f"[{name}] wrote {os.path.relpath(expect_file, ROOT)}" + (f"; {len(lost)} caught failures now lost" if lost else ""))
        sys.exit(1 if lost else 0)
    failed = False
    for k in lost:
        failed = True
        print(f"::error::{name}: PR #{k[0]} {k[3] or '(task)'} in {k[1]}:{k[2]} failed in CI because of the diff and is no longer selected")
    for n, want in expect["sample"].items():
        got = snaps[n]
        added, dropped = sorted(set(got) - set(want)), sorted(set(want) - set(got))
        if added or dropped:
            failed = True
            print(f"{name} PR #{n}: +{len(added)} -{len(dropped)} selected")
            for x in added[:10]:
                print(f"  + {x}")
            for x in dropped[:10]:
                print(f"  - {x}")
    if failed and not lost:
        print(f"::error::{name}: selections changed; review the diff above and run `python3 replay/replay.py check {name} --update`")
    elif not failed:
        log(f"[{name}] check passed")
    sys.exit(1 if failed else 0)


def compare(name, new_date):
    """Markdown: a fresh replay of the window against its committed results. Exit 1 when a real
    failure the committed results catch is missed now."""
    root = os.path.join(HERE, "results", name)
    committed = sorted(d for d in os.listdir(root) if d != new_date and os.path.isdir(os.path.join(root, d)))[-1]

    def load(d, f):
        with open(os.path.join(root, d, f)) as fh:
            return json.load(fh)

    old_s, new_s = load(committed, "summary.json"), load(new_date, "summary.json")
    key = lambda r: (r["pr"], r["module"], r["task"], r["class"])  # noqa: E731
    caught = lambda rows: {key(r) for r in rows if r["attribution"][:1] in "ab" and r["selected"].get("kt-affected")}  # noqa: E731
    lost = sorted(caught(load(committed, "rows.json")) - caught(load(new_date, "rows.json")),
                  key=lambda k: (k[0], k[1], k[2], k[3] or ""))
    o, n = old_s["recall"]["kt-affected"], new_s["recall"]["kt-affected"]
    ot, nt = old_s["selection"]["kt-affected"]["meanTimeShare"], new_s["selection"]["kt-affected"]["meanTimeShare"]
    print(f"### {name}\n")
    print(f"| | committed ({committed}) | this build |\n|---|---:|---:|")
    print(f"| diff-caused failures run | {o['hit']} / {o['of']} | {n['hit']} / {n['of']} |")
    print(f"| test time vs everything | {100 * ot:.1f}% | {100 * nt:.1f}% |\n")
    for k in lost:
        print(f"- lost: PR #{k[0]} `{k[3] or '(task)'}` in `{k[1]}:{k[2]}`")
    if lost:
        print(f"::error::{name}: {len(lost)} real failures the committed results catch are missed", file=sys.stderr)
        sys.exit(1)


def check_sample(prs, repo, size=40):
    """A fixed, varied sample: PRs touching build logic or the catalog, the largest diffs, and an
    even spread of the rest."""
    changed = {p["number"]: git.changed_files(repo, p["base"], p["head"]) for p in prs}
    build = [n for n, fs in changed.items()
             if any(f.endswith((".gradle.kts", ".gradle", ".toml", ".properties")) for f in fs)]
    largest = sorted(changed, key=lambda n: -len(changed[n]))
    picked = list(dict.fromkeys(sorted(build)[:10] + largest[:5]))
    rest = [p["number"] for p in prs if p["number"] not in picked]
    step = max(1, len(rest) // max(1, size - len(picked)))
    picked += rest[::step][: size - len(picked)]
    return sorted(picked)


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

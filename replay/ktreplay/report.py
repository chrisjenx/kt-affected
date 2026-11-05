"""Results files (`summary.json`, `rows.json`, `prs.json`, `report.md`) and the docs table."""

import glob
import json
import os
import statistics

DIFF_CAUSED = ("a", "b", "b-build")
BEGIN = "<!-- replay-results:begin -->"
END = "<!-- replay-results:end -->"


def pct(x):
    return "n/a" if x is None else f"{100 * x:.1f}%"


def summarize(results):
    rows = results["rows"]
    strategies = results["strategies"]
    classed = [r for r in rows if r["class"]]
    counted = [r for r in classed if r["attribution"] in DIFF_CAUSED]
    recall = {}
    for s in strategies:
        hit = sum(1 for r in counted if r["selected"].get(s))
        prs = {}
        for r in counted:
            prs[r["pr"]] = prs.get(r["pr"], True) and bool(r["selected"].get(s))
        recall[s] = {"hit": hit, "of": len(counted), "recall": hit / len(counted) if counted else None,
                     "prsFullyRun": sum(prs.values()), "prsWithFailures": len(prs)}
    by_attr = {}
    by_source = {}
    for r in rows:
        by_attr[r["attribution"]] = by_attr.get(r["attribution"], 0) + 1
        by_source[r["sources"][0]] = by_source.get(r["sources"][0], 0) + 1
    share = {}
    for s in strategies:
        vals = [p["strategies"][s]["share"] for p in results["prs"] if p["strategies"][s]["share"] is not None]
        times = [p["strategies"][s]["timeShare"] for p in results["prs"] if p["strategies"][s]["timeShare"] is not None]
        sel = sum(p["strategies"][s]["classes"] for p in results["prs"])
        tot = sum(p["totalClasses"] for p in results["prs"])
        share[s] = {
            "meanShare": statistics.mean(vals) if vals else None,
            "medianShare": statistics.median(vals) if vals else None,
            "pooledShare": sel / tot if tot else None,
            "meanTimeShare": statistics.mean(times) if times else None,
            "meanClasses": sel / len(results["prs"]) if results["prs"] else None,
        }
    kinds = {}
    for p in results["prs"]:
        kinds[p["mergeKind"]] = kinds.get(p["mergeKind"], 0) + 1
    return {
        "target": results["target"], "repo": results["repo"], "ktAffected": results["ktAffected"],
        "generated": results["generated"], "window": results["window"],
        "prsReplayed": len(results["prs"]), "mergeKinds": kinds,
        "prsWithErrors": sum(1 for p in results["prs"] if p["errors"]),
        "meanTotalClasses": statistics.mean([p["totalClasses"] for p in results["prs"]]) if results["prs"] else None,
        "oracle": dict(results["oracleStats"], runsListed=results["runs"], rows=len(rows),
                       classRows=len(classed), byAttribution=by_attr, byBestSource=by_source,
                       pricedClasses=results["pricedClasses"]),
        "recall": recall, "selection": share,
        "prsWithDiffCausedFailures": len({r["pr"] for r in counted}),
    }


def misses(results):
    """Rows any kt-affected strategy or the module-level baseline does not run, excluding none."""
    out = []
    for r in results["rows"]:
        if not r["class"]:
            continue
        missed = [s for s in results["strategies"] if s != "run-everything" and not r["selected"].get(s)]
        if missed:
            out.append((r, missed))
    return out


def write(out_dir, name, cfg, results):
    os.makedirs(out_dir, exist_ok=True)
    summary = summarize(results)

    def dump(fname, value):
        with open(os.path.join(out_dir, fname), "w") as fh:
            json.dump(value, fh, indent=1, sort_keys=True)
            fh.write("\n")

    dump("summary.json", summary)
    dump("rows.json", [{k: v for k, v in r.items() if k != "occurrences"} | {
        "occurrences": [{k: o[k] for k in ("runId", "attempt", "headSha", "createdAt", "source")} for o in r["occurrences"]]}
        for r in results["rows"]])
    dump("prs.json", results["prs"])
    dump("failures.json", results["failures"])
    with open(os.path.join(out_dir, "report.md"), "w") as fh:
        fh.write(render(summary, results, cfg))


def render(s, results, cfg):
    repo = s["repo"]
    o = s["oracle"]
    L = [f"# Replay: {repo}", "",
         f"kt-affected {s['ktAffected']}, generated {s['generated']}. Merged pull requests "
         f"{s['window']['since']} .. {s['window']['until']}; CI runs from {s['window']['runsSince']}.",
         "", "## Summary", "",
         f"- Pull requests replayed: **{s['prsReplayed']}** (merge kinds: "
         + ", ".join(f"{k} {v}" for k, v in sorted(s["mergeKinds"].items())) + ")",
         f"- Pull requests where a kt-affected call errored: {s['prsWithErrors']}",
         f"- Test classes per merge commit (CI tasks {', '.join(cfg['ci_tasks'])}): "
         f"{s['meanTotalClasses']:.0f} on average" if s["meanTotalClasses"] else "- No test classes found",
         f"- CI runs listed: {o['runsListed']}; failed or retried runs inspected: {o['runsInspected']}; "
         f"failed jobs: {o['jobsFailed']}; logs read: {o['logsRead']} (expired: {o['logsExpired']}); "
         f"report artifacts read: {o['artifactsRead']} (expired: {o['artifactsExpired']})",
         f"- Failure rows on replayed PRs: {o['rows']} ({o['classRows']} with a test class); by attribution: "
         + ", ".join(f"{k} {v}" for k, v in sorted(o["byAttribution"].items()))
         + "; by best source: " + ", ".join(f"{k} {v}" for k, v in sorted(o["byBestSource"].items())),
         f"- Diff-caused failures (a + b): **{s['recall'][results['strategies'][0]]['of']}** on "
         f"{s['prsWithDiffCausedFailures']} PRs",
         "", "## Recall and cost", "",
         "| Strategy | Diff-caused failures run (strict) | PRs with every such failure run | Mean classes selected | Mean share | Median share | Pooled share | Mean time share |",
         "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for st in results["strategies"]:
        rc, sh = s["recall"][st], s["selection"][st]
        rec = f"{rc['hit']} / {rc['of']}" + (f" ({pct(rc['recall'])})" if rc["of"] else "")
        L.append(f"| {st} | {rec} | {rc['prsFullyRun']} / {rc['prsWithFailures']} | {sh['meanClasses']:.1f} | {pct(sh['meanShare'])} | "
                 f"{pct(sh['medianShare'])} | {pct(sh['pooledShare'])} | {pct(sh['meanTimeShare'])} |")
    L += ["", "Run-everything is what CI ran, so its recall is complete by construction; its row "
          "reports what the inventory holds. Shares count test classes of the CI tasks above; time "
          f"shares price {o['pricedClasses']} classes from CI test reports (0 ms for unpriced classes).", ""]

    L += ["## Failure rows", "",
          "Attribution: **a** the failing test's file changed in the PR; **b** the test reaches a changed "
          "declaration (kt-affected path, or a hand check); **b-build** the failure follows from a build or "
          "dependency change in the PR (hand check); **c** unrelated (flake, environment, or caused by "
          "another change), excluded from recall; **review** not yet hand-checked, excluded; "
          "**task-only** a failing test task no source named a class for.", "",
          "| PR | Module | Task | Class | Attribution | By | Sources | " + " | ".join(results["strategies"]) + " | Note |",
          "|---|---|---|---|---|---|---|" + "---|" * len(results["strategies"]) + "---|"]
    for r in sorted(results["rows"], key=lambda r: (r["pr"], str(r["module"]), str(r["task"]), str(r["class"]))):
        sel = " | ".join(r["selected"].get(st) or "**no**" for st in results["strategies"])
        note = r.get("note") or flake_note(r)
        L.append(f"| [#{r['pr']}](https://github.com/{repo}/pull/{r['pr']}) | `{r['module']}` | {r['task']} | "
                 f"`{r['class'] or '-'}` | {r['attribution']} | {r['by']} | {', '.join(r['sources'])} | {sel} | {note} |")

    ms = misses(results)
    L += ["", "## Rows a selector misses", "",
          "Every class row some kt-affected strategy or the module-level baseline does not run, whatever "
          "its attribution, so nothing is excluded unseen.", ""]
    if not ms:
        L.append("None.")
    for r, missed in ms:
        pr = next(p for p in results["prs"] if p["pr"] == r["pr"])
        L += [f"### #{r['pr']} `{r['class']}` ({r['module']}:{r['task']})", "",
              f"- Merge commit `{pr['head']}`, base `{pr['base']}` ({pr['mergeKind']})",
              f"- Missed by: {', '.join(missed)}; attribution **{r['attribution']}** ({r['by']})",
              f"- Test file: {', '.join('`' + f + '`' for f in r['files']) or 'not found at the merge commit'}"
              f"; changed in the PR: {'yes' if r['testChanged'] else 'no'}; in kt-affected's inventory: "
              f"{'yes' if r['inInventory'] else 'no'}",
              f"- Failed on: " + ", ".join(sorted({f"run {o['runId']} attempt {o['attempt']} ({o['source']}, {o['headSha'][:10]})" for o in r["occurrences"]})),
              f"- Flake evidence: {flake_note(r) or 'none'}",
              f"- Changed declarations the test file names: {', '.join('`' + n + '`' for n in r.get('nameHits', [])) or 'none'}",
              f"- Functional changed declarations: {len(r.get('changedDeclarations', []))}"
              + (": " + "; ".join("`" + d + "`" for d in r["changedDeclarations"][:8]) if r.get("changedDeclarations") else "")]
        if r.get("note"):
            L.append(f"- Hand check: {r['note']}")
        L.append("")
    return "\n".join(L) + "\n"


def flake_note(r):
    f = r.get("flake") or {}
    parts = []
    if f.get("otherPrs"):
        parts.append(f"also failed on {len(f['otherPrs'])} other PR(s): " + ", ".join(x.replace("pr:", "#") for x in f["otherPrs"][:6]))
    if f.get("basePushes"):
        parts.append(f"failed on {len(f['basePushes'])} base-branch push run(s)")
    return "; ".join(parts)


def refresh_docs(root):
    """Rewrites the results block of docs/replay.md from the latest results of each target."""
    doc = os.path.join(root, "docs", "replay.md")
    with open(doc) as fh:
        text = fh.read()
    L = [BEGIN, "",
         "| Repository | PRs | Diff-caused failures | kt-affected | kt-affected+assist | Module-level | "
         "kt-affected mean share | Module-level mean share | Results |",
         "|---|---:|---:|---:|---:|---:|---:|---:|---|"]
    for target_dir in sorted(glob.glob(os.path.join(root, "replay", "results", "*"))):
        dates = sorted(d for d in os.listdir(target_dir) if os.path.isdir(os.path.join(target_dir, d)))
        if not dates:
            continue
        with open(os.path.join(target_dir, dates[-1], "summary.json")) as fh:
            s = json.load(fh)
        rc, sh = s["recall"], s["selection"]

        def r(st):
            if st not in rc:
                return "-"
            x = rc[st]
            return f"{x['hit']} / {x['of']}" if x["of"] else "0 / 0"

        rel = os.path.relpath(os.path.join(target_dir, dates[-1], "report.md"), os.path.join(root, "docs"))
        L.append(f"| [{s['repo']}](https://github.com/{s['repo']}) | {s['prsReplayed']} | "
                 f"{rc['kt-affected']['of']} | {r('kt-affected')} | {r('kt-affected+assist')} | {r('module-level')} | "
                 f"{pct(sh['kt-affected+assist' if 'kt-affected+assist' in sh else 'kt-affected']['meanShare'])} | "
                 f"{pct(sh['module-level']['meanShare'])} | [{dates[-1]}]({rel}) |")
    L += ["", END]
    start, end = text.index(BEGIN), text.index(END) + len(END)
    with open(doc, "w") as fh:
        fh.write(text[:start] + "\n".join(L) + text[end:])

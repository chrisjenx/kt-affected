"""Selections per pull request: kt-affected (as shipped, and with a discovery assist when the target
configures one), a module-level baseline, and the full inventory every share is measured against."""

import gzip
import json
import os
import subprocess

from . import git

BUILD_SCRIPTS = ("build.gradle.kts", "build.gradle")
SETTINGS = ("settings.gradle.kts", "settings.gradle")


def assist_settings(work, include_projects):
    """Appends literal `include(...)` lines for the projects a settings loop adds, mirroring the
    common `includeProjects(dir, path, maxDepth)` idiom: a directory with a build script is a
    project and is not descended into; others are searched to `max_depth`."""
    settings = next((s for s in SETTINGS if os.path.exists(os.path.join(work, s))), None)
    if not settings or not include_projects:
        return []
    found = []

    def walk(rel, depth):
        d = os.path.join(work, rel)
        for name in sorted(os.listdir(d)) if os.path.isdir(d) else []:
            child = f"{rel}/{name}"
            if not os.path.isdir(os.path.join(work, child)) or name.startswith((".", "build")):
                continue
            if any(os.path.exists(os.path.join(work, child, b)) for b in BUILD_SCRIPTS):
                found.append(":" + child.replace("/", ":"))
            elif depth > 0:
                walk(child, depth - 1)

    for spec in include_projects:
        walk(spec["dir"], spec.get("max_depth", 1))
    if found:
        kts = settings.endswith(".kts")
        lines = "".join((f'include("{p}")\n' if kts else f"include '{p}'\n") for p in found)
        with open(os.path.join(work, settings), "a") as fh:
            fh.write("\n// kt-affected replay: discovery assist\n" + lines)
    return found


def checkout(work, sha):
    # The work tree is the replay's own scratch clone: nothing in it is anyone's work.
    git.run(work, "checkout", "-q", "-f", "--detach", sha)


def run_replay_bin(binary, work, base, head, cache_dir, inventory_base):
    p = subprocess.run(
        [binary, work, "--base", base, "--head", head, "--cache-dir", cache_dir,
         "--static-discovery", "--inventory-base", inventory_base],
        capture_output=True, text=True,
    )
    if p.returncode != 0:
        return {"explanation": {"error": f"exit {p.returncode}: {p.stderr[-2000:]}"}, "inventory": {"error": "not run"}}
    return json.loads(p.stdout)


def discover(cli, work, cache_dir):
    p = subprocess.run([cli, "discover", "--json", "--static", "--root", work, "--cache-dir", cache_dir],
                       capture_output=True, text=True)
    if p.returncode != 0:
        return {"error": p.stderr[-2000:], "projects": []}
    return json.loads(p.stdout)


def compact_groups(sel):
    """[{module, task, full, total, classes}] from a Selection."""
    return [{"module": g["module"], "task": g["task"], "full": g["full"], "total": g["total"],
             "classes": g["classes"]} for g in sel.get("groups", [])]


def module_level(discovery, changed, inventory):
    """All test tasks of the modules owning a changed file, plus every module depending on one
    (transitively). A changed file outside every module (root build files, an included build,
    the version catalog) affects every module, as affected-module plugins treat it; Markdown and
    `.github/` are ignored."""
    projects = [p for p in discovery.get("projects", []) if p["path"] != ":"]
    dirs = sorted(((p["dir"].rstrip("/") + "/", p["path"]) for p in projects if p["dir"]),
                  key=lambda x: -len(x[0]))
    every = {g["module"] for g in inventory}
    hit = set()
    everything = False
    for f in changed:
        if f.endswith(".md") or f.startswith(".github/"):
            continue
        owner = next((path for d, path in dirs if f.startswith(d)), None)
        if owner is None:
            everything = True
            break
        hit.add(owner)
    if everything:
        return every, True
    rdeps = {}
    for p in discovery.get("projects", []):
        for conf, target in p.get("deps", []):
            c = conf.lower()
            if "detekt" in c or c.startswith("kover") or c.startswith("lint"):
                continue
            rdeps.setdefault(target, set()).add(p["path"])
    todo = list(hit)
    while todo:
        m = todo.pop()
        for r in rdeps.get(m, ()):
            if r not in hit:
                hit.add(r)
                todo.append(r)
    return hit, False


def select_pr(cfg, paths, pr, empty):
    """Selections for one PR, cached at `paths["select"]/<n>.json.gz`."""
    out_file = os.path.join(paths["select"], f"{pr['number']}.json.gz")
    if os.path.exists(out_file):
        with gzip.open(out_file, "rt") as fh:
            return json.load(fh)
    work = paths["repo"]
    checkout(work, pr["head"])
    res = {"number": pr["number"], "base": pr["base"], "head": pr["head"],
           "changedFiles": git.changed_files(work, pr["base"], pr["head"])}
    variants = [("kt-affected", None)]
    if cfg.get("include_projects"):
        variants.append(("kt-affected+assist", cfg["include_projects"]))
    for name, assist in variants:
        if assist:
            res["assistedProjects"] = len(assist_settings(work, assist))
        cache = os.path.join(paths["kta"], name)
        r = run_replay_bin(paths["replay_bin"], work, pr["base"], pr["head"], cache, empty)
        exp = r.get("explanation") or {}
        inv = r.get("inventory") or {}
        sel = exp.get("selection") or {}
        res[name] = {
            "error": exp.get("error") or sel.get("error"),
            "reasons": sel.get("reasons", []),
            "groups": compact_groups(sel),
            "verdicts": exp.get("verdicts", []),
            "paths": {p["test"]: p["steps"] for p in exp.get("paths", [])},
            "inventoryError": inv.get("error"),
            "inventory": compact_groups(inv),
        }
        res[name]["discovery"] = discover(paths["cli"], work, cache)
    checkout(work, pr["head"])  # drops the assist edit
    os.makedirs(paths["select"], exist_ok=True)
    with gzip.open(out_file + ".tmp", "wt") as fh:
        json.dump(res, fh)
    os.replace(out_file + ".tmp", out_file)
    return res

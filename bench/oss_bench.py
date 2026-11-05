#!/usr/bin/env python3
"""Times kt-affected on any Gradle/Kotlin checkout: cold, warm, after a body edit, after a new declaration.

    bench/oss_bench.py --root DIR --bin BIN [--bin-base BIN] [--runs N] [--file REL] [--label NAME] [--json OUT]

Same method as bench/bench.sh, minus the Python selector: every run is a fresh process; wall clock and
peak RSS per run; the median of RUNS is reported. The edited file is restored byte for byte with its
mtime, then one unmeasured run re-settles the cache. With --bin-base, both binaries run the same cases
interleaved on the same machine, so a pull request compares against its base without cross-runner noise.
Static discovery only: the project's Gradle is never run.
"""

import argparse
import json
import os
import platform
import resource
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

CASES = ["cold", "warm", "body", "decl"]


def run_once(cmd: list[str]) -> tuple[float, int]:
    """Wall seconds and peak MB of one fresh process, measured in a throwaway child interpreter."""
    probe = (
        "import resource,subprocess,sys,time,platform;"
        "t=time.perf_counter();r=subprocess.run(sys.argv[1:],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE);"
        "w=time.perf_counter()-t;"
        "m=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss;"
        "m=m if platform.system()=='Darwin' else m*1024;"
        "sys.stderr.write(r.stderr.decode(errors='replace')) if r.returncode else None;"
        "print(w, m//1048576, r.returncode)"
    )
    out = subprocess.run([sys.executable, "-c", probe, *cmd], capture_output=True, text=True)
    wall, mb, code = out.stdout.split()
    if code != "0":
        sys.exit(f"run failed ({code}): {' '.join(cmd)}\n{out.stderr}")
    return float(wall), int(mb)


def pick_file(root: Path) -> str:
    """A deterministic, mid-sized main-source .kt file, so the edit cases touch ordinary code."""
    files = sorted(
        p for p in root.rglob("*.kt")
        if "/build/" not in str(p) and "/src/" in str(p) and "Test" not in str(p)
        and any(s in str(p) for s in ("/commonMain/", "/main/", "/jvmMain/"))
    )
    if not files:
        sys.exit(f"no main-source .kt files under {root}")
    files.sort(key=lambda p: (p.stat().st_size, str(p)))
    return str(files[len(files) // 2].relative_to(root))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True, type=Path)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--bin-base")
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--file")
    ap.add_argument("--label")
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    root = a.root.resolve()
    rel = a.file or pick_file(root)
    target = root / rel
    bins = {"head": str(Path(a.bin).resolve())}
    if a.bin_base:
        bins["base"] = str(Path(a.bin_base).resolve())
    caches = {k: Path(tempfile.mkdtemp(prefix=f"kt-affected-bench-{k}-")) for k in bins}
    orig = Path(tempfile.mkdtemp()) / "orig"
    shutil.copy2(target, orig)

    def cmd(k: str) -> list[str]:
        return [bins[k], "graph", "--root", str(root), "--cache-dir", str(caches[k]), "--static-discovery", "--json"]

    def settle(k: str) -> None:
        subprocess.run(cmd(k), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)

    def restore() -> None:
        shutil.copy2(orig, target)

    samples: dict[tuple[str, str], list[tuple[float, int]]] = {}
    try:
        for k in bins:
            settle(k)
        for case in CASES:
            for _ in range(a.runs):
                for k in bins:  # interleaved, so drift hits both binaries alike
                    if case == "cold":
                        shutil.rmtree(caches[k])
                        caches[k].mkdir()
                    elif case == "body":
                        with open(target, "a") as f:
                            f.write("\n// kt-affected bench body edit\n")
                    elif case == "decl":
                        with open(target, "a") as f:
                            f.write("\nval ktAffectedBenchNewDeclaration = 1\n")
                    samples.setdefault((k, case), []).append(run_once(cmd(k)))
                    if case in ("body", "decl"):
                        restore()
                        settle(k)
        size = json.loads(subprocess.run(cmd("head"), capture_output=True, text=True, check=True).stdout)
    finally:
        restore()
        for c in caches.values():
            shutil.rmtree(c, ignore_errors=True)

    label = a.label or root.name
    rows = []
    for (k, case), s in samples.items():
        rows.append({
            "engine": "rust", "binary": k, "project": label, "case": case, "runs": a.runs,
            "median_s": round(statistics.median(x[0] for x in s), 3),
            "peak_mb": int(statistics.median(x[1] for x in s)), "samples": s,
            "files": size["files"], "nodes": size["nodes"], "edges": size["edges"], "edited": rel,
            "machine": f"{platform.system()} {platform.machine()}, {os.cpu_count()} cores",
            "ts": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        })
    if a.json:
        with open(a.json, "a") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")

    by = {(r["binary"], r["case"]): r for r in rows}
    head = "| Case | time | peak |" + (" base time | change |" if "base" in bins else "")
    print(f"### {label}: {size['files']:,} files, {size['nodes']:,} declarations, {size['edges']:,} edges\n")
    print(head)
    print("|---|---:|---:|" + ("---:|---:|" if "base" in bins else ""))
    for case in CASES:
        h = by[("head", case)]
        line = f"| {case} | {h['median_s']:.3f} s | {h['peak_mb']} MB |"
        if "base" in bins:
            b = by[("base", case)]
            delta = (h["median_s"] - b["median_s"]) / b["median_s"] * 100 if b["median_s"] else 0.0
            line += f" {b['median_s']:.3f} s | {delta:+.0f}% |"
        print(line)
    print(f"\nMedian of {a.runs} fresh-process runs; edits append to `{rel}`.")


if __name__ == "__main__":
    main()

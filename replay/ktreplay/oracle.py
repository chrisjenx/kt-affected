"""The CI failure oracle: which test classes failed, in which Gradle task, on which run.

Sources, most to least exact: JUnit XML in artifacts, Gradle HTML test reports in artifacts,
check-run annotations, job logs. Every failure keeps the sources that reported it.
"""

import datetime
import fnmatch
import html
import io
import os
import re
import sys
import xml.etree.ElementTree as ET
import zipfile

SOURCE_RANK = {"junit-xml": 0, "html-report": 1, "annotation": 2, "log": 3}

TS = re.compile(r"^\d{4}-\d\d-\d\dT[\d:.]+Z ?")
ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
TASK_HDR = re.compile(r"^> Task (:[\w:.\-]+)(?: (?:FAILED|UP-TO-DATE|NO-SOURCE|SKIPPED|FROM-CACHE))?\s*$")
TEST_FAILED = re.compile(r"^([A-Za-z_][\w.$]*(?:\[[^\]]*\])?) > (.+) FAILED\s*$")
EXEC_FAILED = re.compile(r"Execution failed for task '(:[^']+)'")
FAILING_TESTS_REPORT = re.compile(r"file://\S*?/build/reports/tests/([\w-]+)/index\.html")
REPORT_PATH = re.compile(r"(?:^|/)((?:[^/]+/)*?)build/reports/tests/([\w-]+)/index\.html$")
XML_PATH = re.compile(r"(?:^|/)((?:[^/]+/)*?)build/test-results/([\w-]+)/[^/]*\.xml$")
TR = re.compile(r"<tr>(.*?)</tr>", re.S)
TD = re.compile(r"<td([^>]*)>(.*?)</td>", re.S)
MAX_MEMBER = 20 * 1024 * 1024
MAX_ARTIFACT = 400 * 1024 * 1024


# Aggregate reports, not test tasks: `allTests`, and a KMP target's report over its environments.
AGGREGATE_REPORTS = re.compile(r"^(allTests|jsTest|wasmJsTest|wasmWasiTest)$")


def is_test_task(task):
    name = task.rsplit(":", 1)[-1]
    if AGGREGATE_REPORTS.match(name):
        return False
    if name.startswith(("compile", "assemble", "ksp", "kapt", "generate", "process", "merge")):
        return False
    return name == "test" or "Test" in name or name.endswith("test")


def days(since, until):
    d = datetime.date.fromisoformat(since)
    end = datetime.date.fromisoformat(until)
    while d <= end:
        yield d.isoformat()
        d += datetime.timedelta(days=1)


def list_runs(gh, workflows, since, until):
    """Every run of `workflows` created in [since, until], one query per day (the API returns at
    most 1000 runs per query)."""
    today = datetime.date.today().isoformat()
    runs = []
    for wf in workflows:
        for day in days(since, until):
            ttl = None if day < today else 3 * 3600
            items = gh.paged(f"repos/{gh.repo}/actions/workflows/{wf}/runs?created={day}", "workflow_runs", ttl=ttl)
            for r in items:
                runs.append({
                    "id": r["id"],
                    "workflow": wf,
                    "name": r["name"],
                    "event": r["event"],
                    "headSha": r["head_sha"],
                    "headBranch": r["head_branch"],
                    "headRepo": (r.get("head_repository") or {}).get("full_name"),
                    "prs": [p["number"] for p in r.get("pull_requests") or []
                            if (p.get("base") or {}).get("repo", {}).get("name") == gh.repo.split("/")[1]],
                    "conclusion": r["conclusion"],
                    "status": r["status"],
                    "attempts": r["run_attempt"],
                    "createdAt": r["created_at"],
                })
    return runs


def parse_log(text):
    """Failures in one Gradle job log: [(task or None, class or None, method or None)].

    A `Class > method FAILED` line is attributed to the task of the latest `> Task` header (plain
    console output); without headers (`--quiet`) the task stays None and is resolved later from
    the log's failing test tasks. Each `Execution failed for task` test task also yields a
    task-only failure, so a task whose classes no source names is still counted."""
    task = None
    failures = []
    failed_tasks = []
    for raw in text.splitlines():
        line = ANSI.sub("", TS.sub("", raw)).rstrip()
        m = TASK_HDR.match(line)
        if m:
            task = m.group(1)
            continue
        m = TEST_FAILED.match(line)
        if m and not line.startswith(">"):
            cls = m.group(1)
            if "." not in cls and not cls[:1].isupper():
                continue
            failures.append((task if task and is_test_task(task) else None, cls, m.group(2)))
            continue
        m = EXEC_FAILED.search(line)
        if m and is_test_task(m.group(1)) and m.group(1) not in failed_tasks:
            failed_tasks.append(m.group(1))
    return failures, failed_tasks


def parse_html_report(data):
    """Rows of a Gradle (9.x) test report index: [(class, tests, failures, skipped, ms)]."""
    s = data.decode("utf-8", errors="replace")
    rows = []
    for tr in TR.findall(s):
        cells = TD.findall(tr)
        path = next((html.unescape(re.sub("<[^>]+>", "", v)).strip() for a, v in cells if 'class="path"' in a), None)
        if not path:
            continue
        nums = [re.sub("<[^>]+>", "", v).strip() for a, v in cells if 'class="path"' not in a]
        ms = next((int(m.group(1)) for a, v in cells for m in [re.search(r'data-sort-value="(\d+)"', a)]
                   if m and "%" not in v), None)
        try:
            tests, failures, skipped = int(nums[1]), int(nums[2]), int(nums[3])
        except (IndexError, ValueError):
            continue
        rows.append((path, tests, failures, skipped, ms))
    return rows


def parse_junit_xml(data):
    """[(class, tests, failures, skipped, ms)] for one JUnit XML file."""
    try:
        root = ET.fromstring(data)
    except ET.ParseError:
        return []
    suites = [root] if root.tag == "testsuite" else root.findall("testsuite")
    out = []
    for s in suites:
        per = {}
        for tc in s.findall("testcase"):
            cls = tc.get("classname") or s.get("name")
            t, f, sk, ms = per.get(cls, (0, 0, 0, 0))
            failed = tc.find("failure") is not None or tc.find("error") is not None
            per[cls] = (t + 1, f + failed, sk + (tc.find("skipped") is not None),
                        ms + int(float(tc.get("time") or 0) * 1000))
        out += [(c, *v) for c, v in per.items()]
    return out


def parse_artifact(zip_path):
    """{(module_dir, task): {class: [tests, failures, skipped, ms, source]}} from one artifact zip."""
    found = {}
    try:
        z = zipfile.ZipFile(zip_path)
    except zipfile.BadZipFile:
        return found
    with z:
        for info in z.infolist():
            name = info.filename
            if info.file_size > MAX_MEMBER:
                continue
            m = XML_PATH.search(name)
            kind = "junit-xml"
            if not m:
                m = REPORT_PATH.search(name)
                kind = "html-report"
            if not m:
                continue
            moddir, task = m.group(1).rstrip("/"), m.group(2)
            if AGGREGATE_REPORTS.match(task):
                continue
            data = z.read(info)
            rows = parse_junit_xml(data) if kind == "junit-xml" else parse_html_report(data)
            slot = found.setdefault((moddir, task), {})
            for cls, t, f, sk, ms in rows:
                prev = slot.get(cls)
                if prev is None or SOURCE_RANK[kind] < SOURCE_RANK[prev[4]]:
                    slot[cls] = [t, f, sk, ms, kind]
    return found


def collect(gh, cfg, runs, cache_dir, log=print):
    """Failure events and per-class durations from every run that failed or was retried."""
    events = []
    reports = []  # (run id, {(moddir, task): {class: [...]}}) for pricing
    art_dir = os.path.join(cache_dir, "artifacts")
    todo = [r for r in runs if r["status"] == "completed" and (r["conclusion"] not in ("success", "skipped", "cancelled", None) or r["attempts"] > 1)]
    stats = {"runs": len(runs), "runsInspected": len(todo), "jobsFailed": 0, "logsExpired": 0,
             "logsRead": 0, "artifactsRead": 0, "artifactsExpired": 0}
    for i, r in enumerate(todo):
        if i % 25 == 0:
            log(f"  oracle: run {i + 1}/{len(todo)} ({gh.calls} API calls)")
        base = dict(runId=r["id"], workflow=r["workflow"], event=r["event"], headSha=r["headSha"],
                    createdAt=r["createdAt"])
        for attempt in range(1, r["attempts"] + 1):
            jobs = gh.paged(f"repos/{gh.repo}/actions/runs/{r['id']}/attempts/{attempt}/jobs", "jobs")
            for j in jobs:
                if j.get("conclusion") != "failure":
                    continue
                stats["jobsFailed"] += 1
                ev = dict(base, attempt=attempt, jobId=j["id"], job=j["name"])
                for a in gh.get(f"repos/{gh.repo}/check-runs/{j['id']}/annotations?per_page=100") or []:
                    msg = (a.get("message") or "") + "\n" + (a.get("title") or "")
                    fails, tasks = parse_log(msg)
                    for task, cls, method in fails:
                        events.append(dict(ev, task=task, cls=cls, method=method, source="annotation"))
                    for task in tasks:
                        events.append(dict(ev, task=task, cls=None, method=None, source="annotation"))
                text = gh.text_gz(f"repos/{gh.repo}/actions/jobs/{j['id']}/logs")
                if text is None:
                    stats["logsExpired"] += 1
                    continue
                stats["logsRead"] += 1
                fails, tasks = parse_log(text)
                for task, cls, method in fails:
                    events.append(dict(ev, task=task, cls=cls, method=method, source="log",
                                       failedTasks=tasks))
                for task in tasks:
                    events.append(dict(ev, task=task, cls=None, method=None, source="log"))
        if r["conclusion"] == "success":
            continue
        arts = gh.get(f"repos/{gh.repo}/actions/runs/{r['id']}/artifacts?per_page=100") or {}
        for a in arts.get("artifacts", []):
            if not any(fnmatch.fnmatch(a["name"], p) for p in cfg.get("artifacts", [])):
                continue
            if a["expired"]:
                stats["artifactsExpired"] += 1
                continue
            if a["size_in_bytes"] > MAX_ARTIFACT:
                continue
            dest = os.path.join(art_dir, str(a["id"]), "artifact.zip")
            parsed_cache = os.path.join(art_dir, str(a["id"]), "parsed.json")
            parsed = load_parsed(parsed_cache)
            if parsed is None:
                if not gh.download(f"repos/{gh.repo}/actions/artifacts/{a['id']}/zip", dest):
                    stats["artifactsExpired"] += 1
                    continue
                parsed = parse_artifact(dest)
                save_parsed(parsed_cache, parsed)
                os.remove(dest)  # only the parsed reports are kept
            stats["artifactsRead"] += 1
            reports.append((r["id"], parsed))
            for (moddir, task), classes in parsed.items():
                for cls, (t, f, sk, ms, kind) in classes.items():
                    if f:
                        events.append(dict(base, attempt=None, jobId=None, job=a["name"], moduleDir=moddir,
                                           taskName=task, task=None, cls=cls, method=None, source=kind))
    return events, reports, stats


def load_parsed(path):
    import json
    if not os.path.exists(path):
        return None
    with open(path) as fh:
        raw = json.load(fh)
    return {tuple(k.split("\t")): v for k, v in raw.items()}


def save_parsed(path, parsed):
    import json
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as fh:
        json.dump({f"{k[0]}\t{k[1]}": v for k, v in parsed.items()}, fh)


def eprint(*a):
    print(*a, file=sys.stderr)

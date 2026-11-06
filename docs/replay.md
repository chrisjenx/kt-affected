# Public accuracy replay

The headline numbers in [accuracy.md](accuracy.md) come from a production app that cannot be named,
so nobody else can check them. This replay asks the same question of public repositories, with
nothing but their CI history: **would kt-affected have run the tests that really failed in CI
because of a pull request's diff?** Anyone with `gh` access can rerun it and get the same rows.

## Latest results

<!-- replay-results:begin -->

| Repository | PRs | Diff-caused failures | kt-affected | kt-affected+assist | Module-level | kt-affected mean share | Module-level mean share | Results |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| [slackhq/circuit](https://github.com/slackhq/circuit) | 109 | 45 | 45 / 45 | - | 45 / 45 | 33.2% | 76.9% | [2026-11-06](../replay/results/circuit/2026-11-06/report.md) |
| [element-hq/element-x-android](https://github.com/element-hq/element-x-android) | 501 | 52 | 47 / 52 | 47 / 52 | 51 / 52 | 19.9% | 63.3% | [2026-11-06](../replay/results/element-x-android/2026-11-06/report.md) |

<!-- replay-results:end -->

Run 2026-11-06 with kt-affected 0.4.0 plus two unreleased fixes from the first run's gaps (below).
Static discovery reads element-x-android's settings loop, so the assist row matches the plain one.

**What the misses were, and what was done.** The first run (2026-11-05, 0.4.0) missed 14 rows.
None was a graph miss: every failure a changed declaration reached through code was run. They were
tests that observe a change some other way. Each gap got a corpus case, a fix was tried, and the
fix was kept only if the replay showed it cheap for what it catches:

| Gap | Rows | Corpus case | Outcome |
|---|---:|---|---|
| Source-scanning tests (Konsist rules over every source file: license headers, preview naming, `@Immutable`) failing on a PR's new code | 8 | `source-scanning-test`, `source-scanning-fork` | fixed: a test importing a scanner runs on any Kotlin source change; all 8 caught |
| An Android module's `testReleaseUnitTest` (`testBuildType = "release"`) | 1 | `android-release-unit-tests` | fixed: unit tests run under the module's `testBuildType`; caught |
| A catalog bump of a runtime library (androidx core) made a host-test call throw; one smoke class per task did not cover it | 1 | `catalog-used-library-version` (`known_gap`) | tried and reverted: running whole tasks of the modules declaring a bumped library caught it for +3.5 points of test time on element-x-android and +9.6 on circuit |
| Tests inside a private git submodule whose pointer the PR moved; no sources in a public checkout | 4 | none: not reproducible without the submodule | not fixable from a public checkout |

The two kept fixes cost element-x-android 4.1 points of test time (17.7% to 21.8%) for 8 more
caught failures, and circuit 0.9 points (29.8% to 30.7%) for 1. The production app's replay is
unchanged by them (it uses neither).

Each repository's `report.md` has the per-PR table, every failure row with its provenance and
attribution, and every row a selector missed with the evidence for it.

## Method

1. **Pull requests.** Every pull request merged in the window (default: the last 90 days, which is
   as far back as GitHub keeps job logs). The change is measured as GitHub landed it:
   - merge commit (two parents): base = first parent, head = the merge commit;
   - squash (one parent, the PR's commits not on the branch): base = parent, head = the commit;
   - rebase (the last *n* first-parent commits carry the PR's *n* subjects): base = `merge~n`.
2. **Failure oracle.** Every run of the target's test workflows on the PR (all pushes, every
   attempt, not only the last), plus pushes to the base branch and runs on other PRs, which are the
   flake evidence. For each failed job, in order of exactness:
   - JUnit XML in uploaded artifacts;
   - Gradle HTML test reports in uploaded artifacts (both targets upload these on failure);
   - check-run annotations (they outlive logs);
   - the job log: `Class > test FAILED` lines under the latest `> Task :module:task` header, and
     `Execution failed for task ':module:task'` for each failing test task (`--quiet` builds print
     only this, so the class comes from the report, or from the inventory when one failing task
     holds it).
   Every row keeps every run, attempt, head commit and source that reported it. Compile failures are
   not test failures and are not rows. A failing test task no source names a class for is a
   *task-only* row, counted separately.
3. **Attribution.** Each (PR, module, task, class) row is one of:
   - **a**: the test's file changed in the PR;
   - **b**: the test reaches a changed declaration (a kt-affected `explain` path, or a hand check);
   - **b-build**: the failure follows from a build or dependency change in the PR (hand check);
   - **c**: unrelated. Automatic when the same class also failed on two or more other PRs, or on a
     push to the base branch, within 7 days (the evidence is listed); otherwise by hand check with
     the reason recorded.
   Rows the rules cannot decide are hand-checked; verdicts live in `replay/review/<target>.json`,
   so the check is part of the record. Every row any selector misses is listed in the report with
   its evidence, whatever its attribution.
4. **Selections**, at each PR's head commit, from a checkout of that commit:
   - **kt-affected**: the tool as built from this repository, static discovery (`explain`, through
     the library, see `examples/replay.rs`).
   - **kt-affected+assist**: only for a target whose settings script includes projects in a loop
     static discovery cannot read. `include_projects` in `replay/targets.toml` mirrors that loop as
     literal `include(...)` lines in the checkout, so the rest of the tool can be measured; the plain
     row above is what a user gets today.
   - **module-level**: every test task of each module owning a changed file and of every module
     depending on it, transitively, from the same static discovery. A changed file outside every
     module (root build files, the version catalog, an included build) runs everything; Markdown and
     `.github/` are ignored. This is what affected-module Gradle plugins do.
   - **run-everything**: what CI ran.
   The inventory every share is counted against is kt-affected's own listing of every test class of
   every task (a selection from an empty tree), restricted to the test tasks CI runs on pull requests.
5. **Metrics.** Strict recall: a row counts as run only if the strategy runs that class (or its whole
   task) in the right module *and* the right Gradle task, over rows attributed a, b or b-build.
   Selection size: classes selected per PR, as a share of the inventory (mean, median and pooled
   over PRs). Time share, when CI uploaded full test reports: per-class durations (the median of
   every report seen) summed over the selection, as a share of the inventory's.

What this replay cannot show: a failure CI never saw (a test that would have failed but did not run
because the build stopped first), and over-selection beyond counts and time. Both targets run every
test on every PR, so the oracle is complete for the tasks they run, but small: most pull requests
pass.

## Running it

```bash
python3 replay/replay.py run circuit              # last 90 days; --days N, --until YYYY-MM-DD, --limit N, --workers N
python3 replay/replay.py run element-x-android
python3 replay/replay.py report circuit           # re-analyse cached data (after editing a review file)
python3 replay/replay.py docs                     # refresh the table above from the latest results
```

Needs Python 3.11+, git, a Rust toolchain and an authenticated `gh` (the API budget is 5,000
requests an hour; a 90-day run of either target uses well under that, and the run waits when
limited). Everything is cached under `.replay/` (gitignored): blobless clones, API responses, job
logs (gzipped), parsed report artifacts and each PR's selections, so an interrupted run resumes and
a rerun only fetches what is new. Results go to `replay/results/<target>/<date>/`: `summary.json`,
`rows.json` (every failure row with its provenance), `prs.json` (per-PR selection sizes) and
`report.md`.

Adding a repository is one table in `replay/targets.toml`: its workflows, the test tasks CI runs,
and the artifact names that hold test reports.

## In CI

A full replay is too slow to run on every change, so it runs in three layers, cheapest first:

1. **Corpus cases** (`tests/cases/`, every change, seconds): one small fixture per edge case.
2. **`replay.py check <target>`** (every change, a few minutes): re-selects only the PRs whose real
   CI failures kt-affected catches, and fails if one is lost; and a fixed sample of 40 PRs whose
   selections are committed in `replay/check/<target>.json`, so any change in what is selected shows
   as a per-PR diff of test classes. Accept a reviewed diff with `check <target> --update`.
3. **The full replay** (the `Replay` workflow, after every merge to main and before a release):
   every PR of the window, sharded across runners, scored against the CI failures and compared with
   the committed results. A release does not build if it catches fewer real failures.

All three use the window frozen in `replay/frozen/<target>.json.gz` (PRs, CI runs, failure events,
test reports): no GitHub API, and the same answer every time. A maintainer moves the window with
`run <target> --freeze` (needs `gh`), commits the new frozen file and results, and refreshes the
check with `check <target> --update`.

```bash
python3 replay/replay.py check circuit --workers 4          # the per-change gate
python3 replay/replay.py run circuit --frozen               # the full replay of the frozen window
python3 replay/replay.py run element-x-android --frozen --shard 1/4 --shard-out shards   # one shard
python3 replay/replay.py run element-x-android --frozen --shards-from shards --out-date ci
python3 replay/replay.py compare element-x-android --out-date ci   # vs the committed results
```

Each PR's inventory (every test class per task, the denominator of every share) comes from the
selector's own index (`Explanation::inventory`), so a PR costs one selection, about 2 s.

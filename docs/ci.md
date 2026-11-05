# Using kt-affected in CI and pre-push

## GitHub Actions

```yaml
on: pull_request
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0          # the base commit must be in the clone
      - uses: chrisjenx/kt-affected@v0.3.1
        id: affected
      - name: Run affected tests
        if: steps.affected.outputs.gradle-args != ''
        run: ./gradlew ${{ steps.affected.outputs.gradle-args }}
```

Outputs: `gradle-args` (tasks with `--tests` filters, empty when nothing needs to run) and `plan-file`
(the full JSON plan). Inputs: `version`, `base` (defaults to the pull request's base commit),
`root`, and `args` for extra `select` options such as `--config`.

If kt-affected exits non-zero (exit 3, with `error` set in the JSON plan) the step fails. Treat that
as "run everything", not "run nothing": a selector that cannot answer must never skip tests.

## Any other CI, or a pre-push hook

```bash
curl -fsSL https://raw.githubusercontent.com/chrisjenx/kt-affected/main/scripts/install.sh | sh
args=$(kt-affected select --base "$BASE_SHA" --format gradle) || args="test"   # fail open
[ -z "$args" ] || ./gradlew $args
```

`--base` defaults to the merge-base with `origin/HEAD` (else `origin/main`, else `origin/master`).
Without `--head` the plan covers the work tree, uncommitted changes included, which is what a
pre-push hook wants.

## Output formats

| `--format` | Output |
|---|---|
| `json` (default) | `{base, changedFiles, reasons, groups: [{module, task, classes, full, total, selected, local, ...}], error}` |
| `gradle` | local tasks with `--tests` filters, one line, ready for `./gradlew` |
| `classes` | `:module:task<TAB>fully.qualified.Class`, one per line |

`full: true` means the whole task runs (a fail-open rule fired; `reasons` says which).
`local: false` marks tasks that need a browser, simulator or device; they are in the JSON plan and
left out of `gradle` so a JVM-only runner can skip them.

## Caching the graph between CI runs

The graph lives in `build/kt-affected/` (or `--cache-dir`). Caching it between runs turns a cold
build of the graph into an incremental update; key it on the commit and restore the newest:

```yaml
- uses: actions/cache@v4
  with:
    path: build/kt-affected
    key: kt-affected-${{ github.sha }}
    restore-keys: kt-affected-
```

A cache from a different commit is safe: every file is re-stat'ed and hashed, and anything that
changed is re-parsed.

## Getting the same plan everywhere

CI and pre-push give identical plans when they see the same base, head and config. Pin the binary
(`version: v0.3.1` or `KT_AFFECTED_VERSION`) so a release changes plans only when you upgrade.

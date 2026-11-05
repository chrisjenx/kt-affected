# Accuracy

A test selector is only useful if skipping a test it skips is safe. kt-affected was validated on a
production Kotlin Multiplatform app (14k `.kt` files, 193 Gradle projects) against the selector it
replaced there (a Python script working at the same declaration level) and against what actually
failed in CI, over the app's last 50 merged pull requests.

## Headline (50 merged pull requests)

| Check | kt-affected | previous selector |
|---|---|---|
| Real CI failures selected (right module and task) | **7/7** | 1/7 |
| Classes the previous selector ran that kt-affected skips | 1,044 class-runs on 24 PRs | – |
| ...of those, plausible misses after grading | **0** (10 formatting-only, excused) | – |
| ...dropped samples actually run at the merge commit | 15/15 by cause, plus 5/5 cosmetic drops: all pass | – |
| Classes kt-affected runs that the previous selector skips | 40,235 class-runs on 42 PRs | – |
| Median share of the largest JVM module's tests selected | 2.8% | 2.8% |
| Median share of the largest KMP module's tests selected | 7.4% | 8.8% |
| Test time over the 50 PRs (summed class time) | 36.8 h | 31.2 h (everything: 131.5 h) |

## What the numbers say, and what they don't

- **Skipping is safe on this sample.** Every class kt-affected skips that the previous selector ran
  was graded by a name oracle and a changed-test oracle with no plausible miss, and a sample of each
  narrowing rule was run on the PR's merge commit and passed. That is strong evidence, not proof:
  only a sample of the 1,044 was executed.
- **Recall is higher.** Of the 6 real failures the previous selector missed, 5 were wasm runs of
  shared UI-test classes it mapped only to `jvmTest`; kt-affected maps a source set to every test
  task whose compilation includes it. The sixth was a test that reflects over `X::class`, which
  kt-affected follows. Seven failures is a small sample.
- **Precision of the extra selection is not measured.** The 40,235 extra class-runs are mostly those
  wasm task mappings and DI dispatch through interfaces; they cost 5.6 h more over the 50 PRs. How
  many of them could ever fail is not known. Mutation testing (break each changed declaration, see
  which tests catch it, score each selector) is the planned measure.

## Graph parity

On the same app (296k declarations), kt-affected's graph and the previous selector's agreed on
99.76% of 6.0M edges; every remaining difference class was explained and covered by a test, and most
were bugs in the previous selector.

## Discovery

On the same app, static discovery found 5,004 of the 5,126 project dependencies Gradle reports (the
rest come from a root `kover` aggregation, which it warns about) and no extra ones, plus every test
task that has test sources. Static discovery of pinned okio and sqldelight checkouts is checked in CI
against reviewed goldens (`tests/oss/`).

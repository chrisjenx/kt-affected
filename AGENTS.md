# AGENTS.md

Context for coding agents working on kt-affected. It holds what the code cannot tell you: the goals,
the decisions already made, and how changes are judged. Build, test and release commands are in
`CONTRIBUTING.md`; how the tool works is in `docs/how-it-works.md`.

## Goal

A free, open-source, project-agnostic **test differ** for Kotlin / Kotlin Multiplatform Gradle builds:
given a diff, run only the tests that can observe it, so development gets faster without losing
safety. It started as a Rust rewrite of a Python selector inside a large private production app,
proved there, and was extracted here.

Priorities, in order:

1. **Never miss a test that observes the change.** Skipping a test that would have failed is the
   worst outcome a selector can have. When the tool cannot know, it fails open (runs the whole task
   or module) and says why in `reasons`.
2. **Select as little as possible beyond that.** Over-selection is a cost, never a correctness bug,
   but it is how users judge the tool, so it gets driven down wherever a sound rule allows.
3. **Be fast.** Speed is the project's reason to exist next to Gradle-plugin approaches.

## Decisions already made (change them only with the maintainer)

- **Never run the analysed project's build.** Discovery is static: settings, build scripts, version
  catalogs and convention plugins are read, not executed. `discover --gradle-export` exists only to
  measure static discovery on a project the user trusts; selection must never need it. When static
  reading misses something, make the reader smarter instead of reaching for Gradle.
- **No project-specific code in the core.** Anything particular to one codebase belongs in
  `kt-affected.toml` (see `docs/configuration.md`) with a generic default.
- **Change classification is semantic.** A cosmetic change (formatting, comments, KDoc, `@Suppress`,
  local renames...) seeds nothing; a functional one propagates through every downstream path, DI
  included. Precision comes from that classification and from typed dispatch, not from cutting
  graph edges.
- **A narrowing needs evidence.** Any rule that selects *less* must come with (a) a corpus case
  showing the dropped test cannot observe the change, and (b) for a broad rule, a replay over real
  pull requests showing no real CI failure is lost and that sampled dropped tests pass at their merge
  commits. A rule that loses a real, diff-caused failure is reverted, whatever it saves.
- **Build logic that changes how tests run** (test-JVM settings, a codegen or compiler-plugin version
  bump, see `[selector] codegen_plugins`) runs whole tasks. Other build-logic edits run one smoke
  class per task. This was measured: running everything on every build-logic change costs about 20x
  more for no extra caught failures.
- **Multi-target mapping is correct, even though it costs.** A shared test source set runs under
  every test task whose compilation includes it (jvm, wasm, js, iOS, Android). That is where most of
  the extra selection comes from, and it is also what catches failures a jvm-only mapping misses.
  Projects mark tasks a local loop doesn't run as non-local with `[tasks] local` (runners defer
  them); `[tasks] only` excludes tasks entirely (dropped with a reason). The tool does not guess.
- **Licence:** dual MIT / Apache-2.0. **MSRV** follows kt-syntax's (`rust-version` in `Cargo.toml`,
  pinned in CI).
- **kt-syntax** (from [ktrs](https://github.com/Hexay/ktrs)) is a git dependency pinned to a rev.
  Bumping it can change parse trees, so re-run the corpus and the discovery goldens.

## Performance budget

Measured on a 14k-file, 193-project KMP app with a prebuilt release binary (compile excluded):
cold graph < 5 s, after a one-file edit < 300 ms, warm (nothing changed) < 150 ms and < 150 MB peak.
Current numbers are in the README. A change that slows these needs a reason; the `Benchmark` workflow
compares every pull request with its base on the same runner (okio and sqldelight).

## How accuracy is judged

- **Recall, strict:** of the test failures a diff really caused in CI, how many the selection would
  have run in the right module *and* the right Gradle task. Flakes, time bombs and environment
  failures are excluded by hand-check, with the reason recorded.
- **Cost:** summed per-class test time selected, as a share of running every test.
- The headline numbers in `README.md` and `docs/accuracy.md` come from a replay of every merged pull
  request of a private production app since its selector was introduced (586 at the last run). The
  maintainer runs that replay outside this repo; update those numbers only from a new run, and keep
  the improvement table in `docs/accuracy.md` in step.
- `tests/cases/` is the public, permanent record of every edge case learned. Every selection fix adds
  a case; a gap found but not yet fixed is added with `known_gap` (the harness fails once it passes).

## Privacy rule

This repo is meant to be public. **Never add anything that identifies the private app used for
validation**: no names, package names, module names, test or class names, PR numbers, commit SHAs or
CI output, in code, fixtures, comments, docs, commit messages or PR descriptions. Describe it as "a
production app". Fixtures use neutral domains (Book, Author, Order, Invoice...). The repo's history
was rewritten once to remove such material; keep it clean.

## Known open items

- Interface fan-out to fakes and unrelated holders over-selects; narrowing it needs tracking which
  implementation flows into which holder, which the graph does not record.
- Event routing through a sealed event base is only partly safe to narrow: some referrers read just
  the event's companion constants, and the graph does not mark companion-only references.
- A test method with the same name as the member it calls (`fun lines()` calling `x.lines()`) is not
  picked up through an untyped receiver.
- Accuracy beyond recall is not yet measured; mutation testing (break each changed declaration, see
  which tests catch it, score selectors) is the planned measure.

## Library

The crate is a library (`kt_affected`) with the CLI as a thin binary over it (`docs/library.md`). The
public API is `lib.rs` alone; every other module stays private, and public structs and enums are
`#[non_exhaustive]` so fields can be added without a breaking release. A long-lived process links
it, so a call keeps nothing resident: no growing `static` state, nothing leaked with
`mem::forget`, per-call state passed down (as `Routes::explain_via` is). The CLI's output must stay
byte-identical across a refactor of this kind; compare it against the previous release's binary on
okio and sqldelight.

## Conventions

- Each fix is its own commit, with the corpus case named in the subject and, for anything that
  changes selection on real projects, the measured effect in the message.
- Merge stacked branches with a merge commit (not squash), so branches built on them stay valid.
- Never force-push `main`.

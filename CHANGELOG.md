# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/); before 1.0, a minor version may change
selection results or the plan format.

## [Unreleased]

- Static discovery reads settings scripts that include projects through code (a directory walk, a
  project DSL): every directory holding a build script, outside nested and included builds, is a
  project, named the way the build scripts' own project references resolve (`:a:b` or the leaf
  `:b`). element-x-android goes from 14 to 359 projects, ktor from 1 to 138.
- The flattened `<stem>/src`, `<stem>/test` source layout (build logic calling
  `kotlin.setSrcDirs`, as ktor and the kotlinx libraries do) is read as source sets; before, a
  functional edit in such a build selected nothing.
- An Android module with product flavors runs its unit tests under `test<Flavor>DebugUnitTest`
  (it has no `testDebugUnitTest`), and a variant's own test source sets (`testPaid`) are tests.
- Per-project script and convention-plugin reading runs in parallel (ktor discovery about 7x
  faster).
- Discovery goldens and the CI benchmark cover five more pinned builds: kotlinconf-app, Confetti,
  circuit, element-x-android and ktor.
- A public accuracy replay (`replay/`, [docs/replay.md](docs/replay.md)): every merged PR of
  element-x-android and circuit over 90 days, scored against the test failures their CI really
  saw. It found three gaps, recorded as `known_gap` corpus cases: source-scanning tests (Konsist),
  a catalog bump of a library a module declares, and Android `testReleaseUnitTest`.

## [0.3.1]

- Several processes or threads can now update one cache dir at once (a Gradle task and an editor
  server both selecting after an edit, say). Before, they shared one temp file name, so one could
  rename another's half-written `graph.bin` or `parses.bin` into place (seen as `No such file or
  directory` errors, and able to leave a torn cache), and two writers appending to `parses.bin`
  could leave records at offsets the graph did not expect. Updates now take turns on an exclusive
  lock on `<cache>/lock`, held from the file walk to the last rename; a caller that waited finds
  the cache fresh and does no work. Readers need no lock. A lock held for over 5 minutes is an
  error ("run everything"), and the OS releases the lock of a process that dies.
- Temp files are unique per writer, and ones a killed writer left are removed by the next update.
- `graph.bin` records which `parses.bin` it was built with (a lineage id written at the start of
  `parses.bin`, and its length). A pair from different updates, or one a crash left half-replaced,
  now rebuilds cold instead of linking against the wrong records. The cache format changed, so
  the first run after upgrading rebuilds the graph.

## [0.3.0]

kt-affected is now a library (`kt_affected`) with the CLI as a thin binary over it. Selection
results and the CLI's output are unchanged: `select` (json, gradle, classes), `explain`, `discover`,
`dump-edges`, `verify` and `init-script` print byte-identical output to 0.2.1 on okio and
sqldelight.

- Library API: `select`, `explain` and `default_base`, with `SelectOptions`, `Selection` (the
  `select --format json` shape, serde both ways, `to_gradle_args()`, `to_classes()`), `Explanation`
  (verdicts and test paths as data, `render()` for the `explain` text) and an `Error` enum. A
  selection that cannot be made is always an `Err`, never an empty plan. Public types are
  `#[non_exhaustive]`. See [docs/library.md](docs/library.md).
- Depend on it with `kt-affected = { git = "https://github.com/chrisjenx/kt-affected", tag =
  "v0.3.0", default-features = false }`. The binary needs the default `cli` feature.
- A call keeps nothing resident: `explain`'s path capture moved out of process-wide statics into
  the call, and a graph rebuild no longer leaks the linker's scratch memory (`mem::forget`) to save
  freeing it.
- Memory of a library call (`examples/rss.rs`, fresh process, median of 5): okio 41.3 MB cold,
  12.0 MB warm; sqldelight 45.6 MB cold, 11.8 MB warm. 100 warm calls in one process: RSS
  11.1 → 14.6 MB at call 20 → 16.9 MB at call 100 on okio (sqldelight 10.8 → 12.6 → 13.8 MB),
  live heap holding at about 2 MB.
- `select --root` on a directory below the build root now resolves to the build, as without
  `--root`; a directory in no Gradle build exits 3 with `error` set instead of printing an empty
  plan. The `select:` timing line on stderr now covers the whole call, graph update included.

## [0.2.1]

- `[tasks] only` no longer fails a module open when it removes every task of one of its test
  source sets (`iosTest` or `jsTest` with only `jvmTest` kept). Such a source set is dropped with a
  reason, and neither the module nor its dependents run whole; no `*` group is emitted. A source set
  mapped to no task at all still fails open as before.
- Tasks `[tasks] only` removes are listed in `reasons` for the modules a plan touches.

## [0.2.0]

On a replay of 586 pull requests of a production app, this release would have run 99 of the 103
test failures those diffs caused in CI (0.1.0: 91), at 15.1% of the full test time. See
[docs/accuracy.md](docs/accuracy.md).

Selection changes from the edge-case corpus (every former known gap now passes):

- More selected (missed tests before): accessors declared on a DI graph itself, fully
  qualified names with no import, Kotlin in an included plugin build (build logic) or an
  included library build (code), named KMP targets (`jvm("desktop")` -> `desktopTest`).
- Less selected, each narrowing argued sound in its commit: a private class member links
  only inside its top-level declaration; a test whose DI graphs all exclude the edited
  implementation; a platform-only (`jvmMain`) edit under other targets' tasks; event routing
  through a template-method base and a shared handler registry.
- The graph cache format is bumped (6): the first run after upgrading rebuilds cold.

Selection changes from a full-history validation on a production app:

- More selected: build logic that sets a Test task's JVM, framework or forking (`jvmArgs`,
  heap, `systemProperty`, forks, toolchain, test framework) runs every class of the tasks it
  configures, and a version-catalog bump of a compiler plugin or code generator (new
  `[selector] codegen_plugins`) runs every task whole; calls on receivers the linker cannot
  type are followed hop after hop, not only from the edited class.
- Less selected: an `init {}` block or constructor is never linked as an override of a
  supertype's; a registered Test task's `filter { includeTestsMatching(..) }` is honoured
  (discovery records it as `test_filters`); a new or changed abstract accessor on a DI graph
  reaches only code reading it.
- Corpus expectations gain `smoke_tasks`: groups that must run some classes but not whole.

## [0.1.0]

First public release.

- Static Gradle/KMP discovery: settings includes, `projectDir`, `includeBuild`, `project(":x")` and
  typesafe `projects.x` deps, version catalogs, convention plugins, the KMP default hierarchy and
  test task mapping. `discover --gradle-export` measures it against Gradle.
- Incremental declaration graph built with kt-syntax, memory-mapped; `verify` checks it against a
  cold rebuild.
- `select` and `explain`: hunk-level seeds, functional-vs-cosmetic classification, typed DI dispatch
  (Metro, Dagger, Hilt, Koin), container gate, event-type routing, resource and string-literal
  routing, fail-open rules with reasons. Output as JSON, Gradle arguments or class lists.
- `--base` defaults to the merge-base with `origin/HEAD` (else `origin/main`, else `origin/master`).
- GitHub Action, install script, release archives for Linux, macOS and Windows.

[Unreleased]: https://github.com/chrisjenx/kt-affected/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/chrisjenx/kt-affected/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/chrisjenx/kt-affected/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/chrisjenx/kt-affected/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/chrisjenx/kt-affected/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/chrisjenx/kt-affected/releases/tag/v0.1.0

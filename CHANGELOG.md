# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/); before 1.0, a minor version may change
selection results or the plan format.

## [Unreleased]

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

[Unreleased]: https://github.com/chrisjenx/kt-affected/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/chrisjenx/kt-affected/releases/tag/v0.1.0

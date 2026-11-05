# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/); before 1.0, a minor version may change
selection results or the plan format.

## [Unreleased]

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

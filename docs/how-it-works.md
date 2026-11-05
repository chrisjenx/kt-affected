# How it works

kt-affected answers one question: given a diff, which test classes can observe it? It gets there in
four steps, each cached so the next run only redoes what changed.

1. **Discover the build.** Read `settings.gradle(.kts)`, every build script, version catalogs and
   convention plugins, without running Gradle, to learn the projects, their dependencies, KMP source
   sets and the test task that runs each test source set.
2. **Parse every `.kt` file** with [kt-syntax](https://github.com/Hexay/ktrs), a lossless Kotlin
   parser whose trees match the compiler's PSI, in parallel.
3. **Link a declaration graph**: an edge from each declaration to every declaration it references,
   resolved with imports, packages, supertypes, implicit receivers and DI bindings. The graph is a
   memory-mapped CSR file (`build/kt-affected/graph.bin`), so a warm run is a stat sweep plus an mmap.
4. **Select**: seed the declarations a diff's hunks touch, classify each as functional or cosmetic,
   and walk the graph backwards to the test classes that can reach a functional seed. Group them by
   the Gradle test task that runs them.

## The pieces

- **Discovers the build without running it**: `settings.gradle(.kts)` includes, `projectDir`
  overrides, `includeBuild`, `project(":x")` and typesafe `projects.x` dependencies with their
  configuration and KMP source set, the KMP default hierarchy plus explicit `dependsOn`, and the test
  task that runs each test source set.
- **Asks Gradle when static reading can't know**: convention plugins add dependencies and targets no
  build script names. `kt-affected discover --gradle-export` runs a configuration-cache-safe init
  script (`gradle/kt-affected-export.init.gradle`) and later runs merge its exact deps and test tasks
  for as long as the build scripts are unchanged.
- **Keeps the graph incremental**: a body-only edit relinks one file; a new or removed declaration
  relinks only the files that name it. A warm run is a stat sweep plus a memory map.
- **Selects tests from a diff**: a hunk seeds the declarations whose own lines it touches (comments
  seed nothing); reachability walks references, supertypes (asking "who calls this?", not "who else
  implements it?"), DI bindings and implicit receivers. A test source set runs under every test task
  whose compilation includes it. A member change also reaches code reflecting over its class through
  a class literal (`X::class`). A file outside the Kotlin sources (resources, SQL, SQLDelight, docs)
  seeds the lines that name it; build logic costs one smoke class per test task in reach; wiring no
  name reaches (`META-INF/services`, `.conf`) fails its module open.


## Incremental updates

A body-only edit re-parses one file and relinks only it. A new or removed declaration relinks only
the files that name it. `kt-affected verify` rebuilds cold beside the incremental graph and fails if
they differ; on 33 real edits to a production app (and their reverts) the two were identical.

## Functional vs cosmetic changes

Each touched declaration is classified before it seeds. Its base and head syntax trees are compared on
their own tokens (nested declarations count as one placeholder each). If they match once formatting,
comments and KDoc, `cosmetic_annotations`, trailing commas, parentheses around a single name or
literal, `this.` before a property no local name shadows, and consistent renames of locals, lambda,
loop and catch parameters and parameters of private or local functions are normalised away, the
change is cosmetic and seeds nothing. Everything else is functional: a public parameter rename
(callers may pass it by name), any literal, an explicit-vs-inferred type, reordered members, a file
that does not parse.

`kt-affected explain --base REF` prints, for each changed declaration, its verdict and why, and for
each selected test the path from a seed to the test, edge by edge.

## Dependency injection

A class bound to its interface by a DI framework (Metro, Dagger, Hilt, Koin) is called through the
interface, so a change inside it dispatches to the interface members it overrides and reaches their
callers. Dispatch is typed: from `Impl.m` it goes to the interface members `m` overrides, then to
code whose call resolves to one of them, and to code that holds the interface (a property or
accessor of its type, through up to three wrappers) and writes the name `m`. It runs to a fixpoint.
A test that can only hold a fake of the interface (its DI graph `excludes` the implementation) does
not observe the change and is not selected. Details and knobs: [configuration](configuration.md).

## What it never does

- Run your Gradle build. Discovery is static. `discover --gradle-export` exists to *measure* static
  discovery against Gradle on a project you trust, and is never needed to select tests.
- Guess silently. When it cannot know (wiring no name reaches, such as `META-INF/services`, or a
  build-logic change), it fails open for the module or task in reach and says why in `reasons`.

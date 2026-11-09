# Configuration

Optional `kt-affected.toml` at the build root (or `--config FILE`). Every key has a generic default;
a repo with no file gets a sound selection. A change to the config file itself selects every test.

```toml
[selector]
self_paths = ["tools/my-rules/"]          # a change here selects every test (trailing / = prefix)
ignored_prefixes = ["docs/", ".github/"]  # never selected for… unless a test names the file in a string
doc_suffixes = [".md"]
outside_module_ignored_suffixes = [".txt"]
module_ignored_basenames = ["detekt-baseline.xml"]
build_logic_globs = ["gradle/plugins/src/main"]  # extra sources whose text decides "is this a build input?"
classpath_bound = true                    # a test runs edited code only with an edited module on its classpath
codegen_plugins = ["dev.zacsweers.metro"] # a version-catalog bump of one of these runs every test task whole
                                          # (default: Kotlin, Compose, KSP, Metro, SQLDelight, Anvil, Dagger, Room, ...)

[database]
migration_dirs = ["app/src/main/resources/db/migration/"]  # Flyway: seed Kotlin naming the tables
module_marker = "useTestDatabase()"       # a build script containing this runs a database in tests
synced_inputs = ["app/src/test/resources/logback-test.xml"]  # copied onto those modules' classpath
test_markers = ["TestDatabase"]           # a database-backed smoke class names one of these

[modules]
direct_only = [":integration"]            # runs only for a change under the module's directory
fail_open_forks = [":app"]                # reported to the runner: a full run needs parallel forks
manual = [":screenshots"]                 # never selected

[tasks]
local = ["test", "jvmTest"]               # default: JVM-hosted tasks are local
only = []                                 # drop every other task, with a reason (empty: all)
[tasks.source_sets]
uiTest = ["jvmTest", "wasmJsBrowserTest"] # override discovery for a source set

[[source_scanner]]                        # in-house tests that read sources as text (Konsist tests
                                          # are found by their import, no config needed)
changed_path = '^app/src/main/.+\.kt$'
test_package = "com.example.guards"       # every test class in the package and its subpackages

[[source_scanner]]
changed_path = '^[^/]+/src/main/.+\.kt$'
exclude_path = '^backend'                 # optional: matching paths never trigger it (no lookahead in `regex`)
test_classes = ["com.example.guards.CborGuardTest"]  # these classes only; exclusive with test_package

[[dispatch]]                              # Handler<E> runs only when an E is dispatched
bases = ["EventHandler"]                  # also through generic bases: Foo : Mid<X>, Mid<E> : EventHandler<E>
construction_smoke = true                 # a construction change keeps one DB smoke class per module

[container_gate]                          # on by default
enabled = true
graph_annotations = ["DependencyGraph", "Component"]  # DI graphs (these and the other Metro/Dagger ones are the default)
graph_types = ["MyTestExtension"]         # graph-like types no annotation marks

[di]                                      # Metro/Dagger/Hilt defaults; empty both lists to turn off
binding_annotations = ["ContributesBinding", "ContributesIntoSet", "ContributesIntoMap", "Inject"]
provider_annotations = ["Provides", "Binds"]  # the classes such a function references are bound
dispatch_rounds = 64                      # safety cap; dispatch runs to a fixpoint (large apps converge in a few rounds) and warns if capped
dispatch = "typed"                        # "names": also keep any test code writing the member's name

[changes]                                 # on by default
classify = true                           # false: every touched declaration seeds
cosmetic_annotations = ["Suppress"]       # adding, removing or editing these never changes behaviour
```

`[tasks] local` and `[tasks] only` cut what runs in different ways. `local` keeps every group in the
plan and marks each with `local`, so a runner can run the local ones now and defer the rest (to CI,
say) without losing them. `only` removes tasks from the plan: a task not listed is never emitted, and
the plan says so in `reasons` (`:core: iosSimulatorArm64Test not run (excluded by [tasks] only)`). A
test source set whose every task `only` removes (`iosTest` when only `jvmTest` is kept) is dropped
the same way, with a reason, instead of failing its module open. Use `local` to defer tasks a local
loop does not run; use `only` to exclude tasks you never want selected. A source set mapped to no
task at all, without `only` removing any, still runs its module and every dependent whole.

A `[[source_scanner]]` is a test that reads sources as text, so the graph never reaches it. A
changed path matching `changed_path`, and not `exclude_path` when one is set, runs its tests: every
test class in `test_package` (and its subpackages), or exactly the fully qualified `test_classes`.
Set one of the two; a config setting both, or neither, fails to load, as does a regex that does not
compile. `exclude_path` exists because the `regex` crate has no lookahead: "every production source
outside `backend*`" is `changed_path = '^[^/]+/src/main/'` plus `exclude_path = '^backend'`.

## Included builds

Each `includeBuild("dir")` in the settings script is discovered like the root build, and its test
source sets become test tasks of the plan. A group in one has `module` set to the composite path,
`:<build name>` for the included build's root project and `:<build name>:<project>` for its
subprojects, and `build` set to the included build's directory. The build name is the directory's
name unless `includeBuild("dir") { name = "x" }` sets one, as Gradle names it. `--format gradle`
prints the composite form, which the root build runs directly:

```bash
./gradlew :plugins:test --tests demo.plugins.BuildVersionTest
```

To run it inside the included build instead, drop the first segment of `module` and pass `-p`:
`./gradlew -p gradle/plugins :test --tests ...`. The JSON has `build` only on such groups.

What selects them:

- the graph: a declaration an included build's test reaches changes (its own code, or a library
  build's code the root build also uses);
- a mention: an ignored or doc path (`.github/workflows/deploy.yml`) that a test there names in a
  string, as for any test;
- an edit to one of its tests, which runs that test and nothing in the root build.

A change to a plugin build's main code is still build logic for the root build (one smoke class per
task in reach), and runs the included build's tests that reach it as well. A change to anything
else in an included build, its build script say, runs one smoke class of its own tasks besides
whatever the root build's rules select. The graph's module visibility does not model included builds,
so linking and the root build's selection are as before. A name collision (two included builds, or
an included build and a root-build project, with the same name) leaves that build out, with a
discovery warning.

A member edit also walks from the member's class to every member of the class's supertypes, so a
body change to `RepoImpl.update` would reach every caller of `Repo.insert`, and every test holding a
`Repo` through a DI graph accessor or a seed fixture. `container_gate` keeps only what can observe
the edit: a supertype member whose code writes a changed member's name (a template method calling it
on `this`), and, when no such member exists, holders of the type that write the name. It is off for
a change to a whole type, construction-time code, an implicitly called member (operators,
`toString`, `hashCode`), an interface member (a contract every implementer and mock sees) or
anything inside a DI graph, whose wiring every user of the graph observes.

A class bound to its interface by a DI framework (`[di]`) is called through the interface, so code
that runs a change inside one dispatches to the interface members it overrides: their callers are
reached, as for a Koin-provided class. A dispatch round is typed: from `Impl.m` it goes to the
interface members `m` overrides, then to code whose call resolves to one of them, and to code that
reads a holder of the interface (a property or accessor of its type, through up to three wrappers)
and writes the name `m`. A caller of `m` on an unrelated type is never reached. Each bound service
reached that way dispatches on until nothing new is reached; `dispatch_rounds` only caps that, and hitting
the cap prints a warning rather than silently cutting the selection short.

Two facts bound what those rounds keep. A test runs edited code only if an edited module is in its
module's dependency closure (`[selector] classpath_bound`). And a test whose every DI graph lists the
dispatching implementation in `excludes = [...]` binds a fake instead, so it cannot observe the
implementation's change. A test that reaches no graph is kept.

Each touched declaration is classified before it seeds (`[changes]`). Its base and head syntax
trees are compared on their own tokens (nested declarations count as one placeholder each). If they
match once formatting, comments and KDoc, `cosmetic_annotations`, trailing commas, parentheses around
a single name or literal, `this.` before a property no local name shadows, and consistent renames of
locals, lambda, loop and catch parameters and parameters of private or local functions are
normalised away, the change is cosmetic and seeds nothing, through DI or anything else. Everything
else is functional: a public parameter rename (callers may pass it by name), any literal, an
explicit-vs-inferred type, reordered members, a file that does not parse. `explain` prints which.


## Measuring static discovery against Gradle


Static discovery reads build scripts only; convention plugins in an included build can add
dependencies and targets it cannot see. To measure that gap on a project you trust:

```bash
cd your-project
kt-affected discover --gradle-export      # runs ./gradlew with the shipped init script, compares static vs Gradle
```

The comparison lists projects and project-dependency edges found by only one side and predicted
test tasks Gradle does not have. Afterwards the export is merged into every run until a build script
changes.


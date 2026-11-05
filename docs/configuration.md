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
only = []                                 # restrict emitted tasks (empty: all)
[tasks.source_sets]
uiTest = ["jvmTest", "wasmJsBrowserTest"] # override discovery for a source set

[[source_scanner]]                        # tests that read sources as text
changed_path = '^app/src/main/.+\.kt$'
test_package = "com.example.guards"

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


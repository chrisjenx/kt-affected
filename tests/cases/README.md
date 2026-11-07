# Selection edge cases

Each directory is one scenario the selector must keep getting right. `tests/cases.rs` copies
`base/` into a scratch git repo and commits it, lays `head/` over it and commits again, runs
`kt-affected select --base HEAD~1 --head HEAD`, and checks the plan against `expect.toml`.

## Layout

```
<case>/
  base/          a small Gradle build: settings, build scripts, Kotlin sources and tests
  head/          only the files the change touches; a file holding exactly `<deleted>` deletes it
  expect.toml
```

`expect.toml` keys (all optional except `why`):

| key | meaning |
|-----|---------|
| `why` | one line: the behaviour the case pins |
| `must` | classes that must be selected; `pkg.Cls@:module:task` pins the task |
| `never` | classes that must not be selected (same `@` form) |
| `full_tasks` | `:module:task` groups that must run whole |
| `tasks` | `:module:task` groups that must run at least one class (a smoke class) |
| `smoke_tasks` | `:module:task` groups that must run some classes but not the whole task |
| `never_tasks` | `:module:task` groups that must not appear |
| `none` | `true`: nothing at all is selected |
| `reasons` | each must be a substring of some reason in the plan |
| `config` | `kt-affected.toml` text, written outside the repo (one inside is a selector change) |
| `known_gap` | the tool gets this wrong today; a failure is printed, not fatal, and a pass fails until the mark is removed |

Run one or a few with `KT_CASES=name,name cargo test --test cases`; add `KT_CASES_SHOW=1` and
`-- --nocapture` to print every plan and the known gaps.

## Cases

| case | pins |
|------|------|
| `android-library-template-group` | Under the KMP Android library plugin, a hierarchy template group's tests run under testAndroidHostTest when the group takes AGP's withAndroid() or the build adds androidHostTest.dependsOn(group), never through withAndroidTarget(), which matches only androidTarget(). |
| `android-product-flavors` | An Android module with product flavors has no testDebugUnitTest: the shared test source set runs under every flavor's test<Flavor>DebugUnitTest, a flavor's own test source set only under that flavor's. |
| `android-res` | A changed Android res/drawable file seeds code naming R.drawable.<stem>, not every user of R.drawable. |
| `android-target-template-group` | With androidTarget(), a withAndroidTarget() template group and an explicit androidUnitTest.dependsOn both put that source set's tests under testDebugUnitTest. |
| `build-logic-test-jvm-config` | A convention plugin adding JVM arguments to every Test task runs every class of every task: a smoke class cannot show a JVM-wide effect. |
| `build-script-change` | A module build script change runs one smoke class per test task of that module and every module depending on it (CI runs every suite), and nothing in unrelated modules. |
| `catalog-codegen-version` | A version-catalog bump of a compiler plugin or code generator runs every class of every task. |
| `catalog-library-version` | A version-catalog bump of a plain library is build logic: one smoke class per task, not whole tasks. |
| `classpath-bound` | An impl reached through its interface runs only in tests whose classpath holds its module: :api-only sees the interface but not :impl. |
| `compose-common-edit` | A commonMain composable edit runs its commonTest class under every target's test task, and reaches app modules through the composable's callers. |
| `compose-jvm-only-edit` | A jvmMain-only edit runs only JVM tasks: no other target compiles jvmMain. |
| `compose-named-jvm-target` | A JVM target renamed with jvm("desktop") runs desktopMain/desktopTest under desktopTest, never a wasm task. |
| `compose-resources` | A changed composeResources drawable seeds code naming Res.drawable.<stem>; the type directory `drawable` is not a token, so another drawable's user is not selected. |
| `convention-plugin-change` | A buildSrc convention plugin change is build logic for every module: one smoke class per test task, everywhere, with a reason. |
| `cosmetic-edits-select-nothing` | Reformatting, comments, KDoc, a local rename, a trailing comma and @Suppress change no behaviour: nothing is selected. |
| `custom-test-task-filter` | A registered Test task's `filter` is honoured: a class its includeTestsMatching/excludeTestsMatching leaves out never runs there; a filter set in a loop is not read. |
| `deleted-kotlin-file` | Deleting a Kotlin file seeds what still writes its names: the dependents' tests run, others don't. |
| `event-handler-dispatch` | A handler routed by its event type argument, through a generic intermediate base, reaches tests that publish its event and not tests publishing another event through the same bus type. |
| `event-handler-shared-registry` | One registry constructs every handler: a handler body edit still reaches only tests that publish its event. |
| `event-handler-shared-registry-construction` | A routed handler's init block runs wherever the shared registry builds it: every bus user's test runs, whatever event it publishes. |
| `event-handler-template-base-calls-handler-code` | A template base that calls a routed handler's companion directly runs the edit for every handler built on it: tests publishing any of their events run. |
| `event-handler-template-method` | A handler whose generic base is a template method (`handle` calls the abstract `onEvent`) is still routed by its event type: an `onEvent` edit reaches only tests that publish that event. |
| `event-handler-template-method-direct-holder` | A test calling the inherited `handle` on the handler itself, with its event built by a helper elsewhere, still runs an `onEvent` edit; a test publishing another event does not. |
| `expect-actual-actual-edit` | Editing the jvmMain actual reaches a commonTest caller only under jvmTest: the iOS and wasm binaries link their own actuals. |
| `expect-actual-expect-edit` | Editing the expect declaration (and its actuals) reaches commonTest callers under every target's task. |
| `file-move` | A file moved to a new path (content unchanged) still selects the tests of the declarations it held. |
| `flat-source-layout` | Build logic that re-points every source set with kotlin.setSrcDirs to the flattened `<stem>/src`, `<stem>/test` layout: common/test is commonTest and runs under jvmTest when common/src changes; jvm/test, which never reaches the edit, does not. |
| `flyway-migration` | A Flyway migration seeds Kotlin naming its tables in a string (Table("books")), not code on other tables. |
| `fully-qualified-call` | A call written with its fully qualified name and no import (`demo.model.Money(100).format()`) reaches the edited member like an imported one. |
| `functional-edits-select` | A literal change, a public parameter rename (named arguments see it) and an inferred-to-explicit return type are functional. |
| `impl-member-receivers` | An impl member edit reaches tests that can dispatch to it (held in a property, assigned in setUp, handed out by a graph object), not a fake-only test or one calling another member. |
| `include-build-convention` | A convention plugin in a pluginManagement includeBuild is build logic for every module applying it: each test task runs a smoke class, with a reason. |
| `include-build-library` | A composite build's included library (substituted for its coordinates) is code on the app's classpath: an edit to it selects the app tests that call it, not unrelated modules. |
| `include-build-precompiled-script` | A precompiled script plugin in a pluginManagement includeBuild is build logic for every module: each test task runs a smoke class, with a reason. |
| `init-block-not-virtual` | An init block is not virtual: a change running in one subclass's init reaches tests constructing it, never a sibling's through the base class's init. |
| `koin-factory` | Koin `factory<Iface> { Impl() }`: an impl edit reaches tests resolving the interface. |
| `koin-single-bind` | Koin `single { Impl(get()) } bind Iface::class`: an impl member edit reaches tests calling that member through get<Iface>(), not other members or other bindings. |
| `koin-singleof-bind` | Koin `singleOf(::Impl) { bind<Iface>() }`: an impl member edit reaches tests holding the interface by inject(). |
| `meta-inf-services` | A META-INF/services change wires code no name reaches: its module and every dependent run whole, with a reason; unrelated modules don't. |
| `metro-accessor-on-graph` | An accessor declared directly on the @DependencyGraph interface (`val pricing: Pricing`) hands out the bound impl like one inherited from an accessor interface. |
| `metro-contributes-binding` | A @ContributesBinding impl edit reaches tests calling that interface member through a graph accessor, not tests calling another member. |
| `metro-create-graph-type-argument` | A graph created as `createGraph<AppGraph>()`, its type only in the type argument, reaches the bound impl like an explicitly typed one. |
| `metro-graph-accessor-added` | A new abstract accessor on a @DependencyGraph hands out its binding only: tests reading it run, tests building the graph and reading other accessors do not. |
| `metro-graph-excludes-edited-impl` | A test graph that `excludes` the edited impl and binds a fake can never run the edit: its test is not selected, the production graph's is. |
| `metro-graph-excludes-edited-impl-still-named` | A test whose graph excludes the edited impl but that still constructs it, seven helper hops away, runs the edit: it stays selected. |
| `metro-graph-excludes-impl-dependency` | A test graph that `excludes` an impl and binds a fake never runs that impl's dependencies: editing a dependency selects the production graph's test, not the fake graph's. |
| `module-script-test-jvm-config` | A module script changing its own Test task's JVM runs that module whole; dependents get a smoke class; unrelated modules nothing. |
| `plain-resource-stem` | A plain resource seeds code writing its file name; its bare stem as an identifier (`index`) is not a read of the file. |
| `reflection-class-literal` | A test reflecting over `Repo::class` sees a member added to Repo, though it calls no member. |
| `settings-computed-includes` | Settings that include projects by walking directories: every directory holding a build script is a project named by its path, so an edit reaches its own tests and its dependents' through projects.books.catalog, and not an unrelated module's. |
| `settings-project-dsl-leaf-names` | A settings DSL that includes projects by their leaf directory name (`:shop-catalog` in shop-books/shop-catalog): the projects are read from disk under the names the build scripts' own references use, and a directory holding a build script that the settings never names is not a project. |
| `shared-test-source-set` | A custom test source set that jvmTest and wasmJsTest both dependOn runs under both tasks, and not under a target that doesn't include it. |
| `sqldelight-queries` | A .sq change seeds code naming its generated queries (BookQueries/bookQueries), not other .sq files' users. |
| `sqldelight-same-named-holder` | Two stores each hold their own `private val queries`: a .sq change reaching BookStore.queries must not cross to AuthorStore's same-named private property. |
| `stale-target-source-dir` | A leftover iosX64Main directory in a module whose script declares its targets (jvm, iosArm64, iosSimulatorArm64) is not a target: no iosX64Test task exists to run. (known gap) |
| `tasks-only-excluded-source-set` | `[tasks] only` removing every task of a source set drops it with a reason; the module and its dependents are not failed open and no `*` group is emitted. |
| `tasks-only-excluded-target-edit` | An edit only an excluded target compiles (iosMain, with `[tasks] only` keeping jvmTest) selects nothing and says so; it does not fail the module open. |
| `test-graph-accessor` | Services handed out as properties of a test graph object: a service edit reaches only tests that read that accessor, though every test holds the graph. |
| `test-only-change` | Editing a test selects that test class and no other test of the same code. |
| `typesafe-accessors-project-dir` | A module moved by `projectDir` is found at its new directory, and `projects.core.moneyModel` is a dependency on it: its dependent's test runs; a module off its classpath with a same-named class doesn't. |
| `unnamed-receiver-chain` | Calls on receivers the linker cannot type are followed hop after hop: tests calling the outer service run; one calling a same-named member elsewhere does not. |


# Analysis: `unreferenced` and `test-reach`

Both commands read the resolved declaration graph `select` builds (and update it first), so they
see what a word count cannot: which `save` a call binds to, a member at any depth, an
implementation reached through its interface.

```bash
kt-affected unreferenced                    # the whole build
kt-affected unreferenced --base origin/main # only what the change orphaned or added
kt-affected test-reach --json               # tests of nothing; prod code no test reaches
kt-affected test-reach --base origin/main --kover build/reports/kover/report.xml --json
```

Without `--base` they analyse the whole build; there is no default base, unlike `select`. `--head`,
`--git-dir`, `--config`, `--cache-dir` and `--root` work as for `select`. The graph is the work
tree's, as for `select`. Exit 3 means no answer (an error is printed): treat it as "unknown", never
as "nothing found".

## `unreferenced`

Reports declarations, at any depth (top-level declarations, types, members, enum entries,
constructor properties), that nothing references.

- **Inbound counts** are resolved edges from outside the declaration itself, split by where the
  referrer lies: `prod`, `test` (a test source set) or `fixture` (`testFixtures*`).
- **A prod declaration** is reported when no prod code references it. `inbound.test` (or `fixture`)
  can be non-zero: prod code only tests reach.
- **A test or fixture declaration** is reported only when nothing references it.

A declaration is kept, not reported, when:

- **The name gate:** it is a function, property or enum entry and its name is written in code
  outside it, in a module that can see it (for a private member, in its own file). A call through
  a receiver whose type the linker infers (`shelf().map { it.display() }`) gets no edge, only the
  name. `namedIn` lists the source kinds where that happened: a prod declaration only tests name
  stays in the report, as test-only.
- **It is a contract:** an override, a member something overrides, an `actual`, or an `external`
  declaration.
- **DI or dispatch builds it:** a class bound by `[di]` annotations (or a `[[dispatch]]` subclass)
  counts the references to the types it binds as its own. One bound to a type outside the sources
  is an entry.
- **Something calls it by name:**
  - `[unreferenced] entry_annotations`, `entry_names`, `ignored_names` and `entry_supertypes`
    (below);
  - test classes, and test classes inheriting their tests;
  - a name written as a string in its own file (`@MethodSource("widths")`, reflection);
  - a type named in a build script (a string or code: `implementationClass = "a.b.Plugin"`,
    `a.b.Stanza.inject()`), a `META-INF/services` file or an `AndroidManifest.xml`.
- **It is read without its name:**
  - an enum's entries when the enum is used through `entries`, `values()`, `valueOf` or a class
    literal (`@EnumSource(Route::class)`);
  - constructor properties of `data`, `value` and `@Serializable` classes, and of entry-annotated
    ones;
  - companion objects (the class name as a value is the companion; their members are still
    reported).
- **An overload is used:** overloads share a name in one scope, and are reported only together.
- **A file that does not parse could reference it:** what that file's module can see counts as
  referenced, and the file is listed in `unparsed`.

Code in no source set (a reference copy beside the sources) is not reported. Init blocks and
secondary constructors never are.

An operator (`operator fun plus`) is reported with `operator: true`: `a + b` does not link to `plus`
in the graph, so callers usually skip these. Java and other non-Kotlin callers are not read: a Kotlin
declaration only Java calls is reported.

Removing a reported declaration can leave others unreferenced: run again.

### `--base REF`

Only two kinds of declaration are reported, each with `change` set:

- `orphaned`: it existed at the base, and the base version of a file the change touched wrote its
  name in a declaration whose head version no longer does, where that name resolves to it: the
  same file or package, an import of it or its package, or (for a member) code naming its
  containing type. These are the orphans a refactor leaves, where most dead code comes from. A
  member that lost its last caller through an inferred receiver in another package is missed (kept
  quiet), never invented.
- `added`: the change added it (no declaration with its fully qualified name existed in the base
  version of any changed file) and nothing references it.

### Configuration

```toml
[unreferenced]
entry_annotations = ["Test", "BeforeEach", "Preview", "Provides", "Serializable", "..."]  # default: test runners, Compose previews, DI providers, serialization (see below)
entry_names = ["main"]                     # simple names that are entry points anywhere
ignored_names = ["*Fake*", "demo.generated.*"]  # globs over the simple or fully qualified name
entry_supertypes = ["Activity", "Worker"]  # a framework instantiates subtypes of these (as written, through in-repo supertypes)
```

The default `entry_annotations` are the test-runner ones (`Test`, `ParameterizedTest`,
`RepeatedTest`, `TestFactory`, `TestTemplate`, `BeforeTest`, `AfterTest`, `BeforeEach`, `AfterEach`,
`BeforeAll`, `AfterAll`, `Before`, `After`, `BeforeClass`, `AfterClass`, `Rule`, `ClassRule`,
`RegisterExtension`, `RunWith`, `ExtendWith`, `Parameters`) and `Preview`, `Provides`, `Binds`,
`BindsInstance`, `IntoSet`, `IntoMap`, `ElementsIntoSet`, `Multibinds`, `ContributesTo`,
`BindingContainer`, `HiltAndroidApp`, `AndroidEntryPoint`, `Serializable`, `JsExport`, `Keep`.
Setting the key replaces the list. An annotation imported under an alias
(`import kotlin.test.Test as KTest`) counts as the name it imports.

## `test-reach`

Walks forward from every test, and reports:

- `testsReachingNothing`: test classes (`method: null`, every member of the class reaches nothing)
  and test methods (annotated `@Test` and the like) whose walk reaches no prod declaration. These
  test nothing of the product.
- `unreached`: prod declarations no test's walk reaches. They are certainly run by no test. The
  converse does not hold: a reached declaration may still be uncovered.

The walk over-approximates what running a test can execute, so both lists are sound in that one
direction:

- every edge out of a reached declaration;
- every override of a reached member, and every DI-bound implementation of a reached type;
- every function or property whose name reached code writes, in a module it can see (calls
  through inferred receivers have no edge);
- for prod code, the container of a reached member, and what building a reached type runs: init
  blocks, constructors, initialized and constructor properties, enum entries and its implicitly
  called members (operators, `toString`, `hashCode`, `equals`, `componentN`).

A test class's own fields are not followed from one of its methods. The coverage walk starts from
every member of every test class anyway.

`--base REF` limits both lists to declarations the change touches (and test classes it touches).

`--kover FILE` reads a Kover XML report (JaCoCo format) and adds `coverage`: per prod declaration
in scope, its lines, branches and instructions, covered and missed, summed over its lines (nested
declarations included). It is matched to the source by package and file name.

## JSON schema (`"schema": 1`)

Both commands print one JSON object with `--json`. `schema` is bumped on any incompatible change.
Fields may be added without a bump.

```json
{
  "schema": 1,
  "base": "origin/main",
  "declarations": [
    {
      "fqName": "demo.core.Book.shelfMark",
      "kind": "function",
      "file": "core/src/main/kotlin/demo/core/Book.kt",
      "startLine": 6,
      "endLine": 6,
      "visibility": "public",
      "source": "prod",
      "member": true,
      "operator": false,
      "inbound": { "prod": 0, "test": 2, "fixture": 0 },
      "namedIn": ["test"],
      "change": "orphaned"
    }
  ],
  "unparsed": ["app/src/main/kotlin/demo/app/Broken.kt"]
}
```

`unreferenced`: `base` is `null` without `--base`. `change` is present only with one.

| field | |
|---|---|
| `fqName` | `package.Outer.member` (a backticked name as written, without backticks) |
| `kind` | `class`, `interface`, `object`, `typealias`, `enum_entry`, `function`, `property`, `constructor_property` |
| `file`, `startLine`, `endLine` | relative to the build root; 1-based, inclusive |
| `visibility` | `public`, `internal`, `protected`, `private` (as written; none written is `public`) |
| `source` | where it is declared: `prod`, `test`, `fixture` |
| `member` | declared inside a type |
| `operator` | an operator Kotlin calls without its name |
| `inbound` | resolved edge referrers from outside it, by source kind |
| `namedIn` | source kinds whose code writes its name with no edge (`unreferenced` only) |
| `change` | `orphaned` or `added` (with `--base`) |

```json
{
  "schema": 1,
  "base": null,
  "testsReachingNothing": [
    { "class": "demo.pricing.MixedTest", "method": "testsNothing",
      "file": "pricing/src/test/kotlin/demo/pricing/MixedTest.kt", "startLine": 15, "endLine": 18 }
  ],
  "unreached": [ { "fqName": "demo.library.Loans.overdueFine", "kind": "function", "...": "as above" } ],
  "coverage": [
    { "fqName": "demo.pricing.Pricing.total", "kind": "function",
      "file": "pricing/src/main/kotlin/demo/pricing/Pricing.kt", "startLine": 4, "endLine": 4,
      "lines": { "covered": 1, "missed": 0 }, "branches": { "covered": 0, "missed": 0 },
      "instructions": { "covered": 6, "missed": 0 } }
  ]
}
```

## Cost

Each command is one graph update plus one pass over it. On a production app of 14k Kotlin files
(300k declarations, 2.9M edges), with a release binary:

| | warm graph | cold (no cache) |
|---|---|---|
| `graph` | | 2.7 to 3.8 s |
| `unreferenced` | 1.2 to 1.6 s | 1.4x cold `graph` |
| `unreferenced --base` (30 commits) | 2.2 s | |
| `test-reach` | 2.2 s | 1.4x cold `graph` |

Peak memory is 130 to 190 MB.

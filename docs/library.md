# kt-affected as a library

The `kt-affected` crate is a library, `kt_affected`, with the command line as a thin binary over it.
A tool that wants a test plan (an editor plugin, a long-lived server, a custom CI step) can link it
instead of spawning the binary and parsing its JSON.

```toml
[dependencies]
kt-affected = { git = "https://github.com/chrisjenx/kt-affected", tag = "v0.3.1", default-features = false }
```

`default-features = false` leaves out the `cli` feature, which only builds the binary. The library
needs no feature.

Not on crates.io yet: it depends on [kt-syntax](https://github.com/Hexay/ktrs) through a git
dependency, and crates.io accepts a crate only when all its dependencies are published there. Until
kt-syntax is, depend on a tag as above (`publish = false` in `Cargo.toml`).

## API

```rust
pub fn select(root: &Path, opts: &SelectOptions) -> Result<Selection, Error>
pub fn explain(root: &Path, opts: &SelectOptions) -> Result<Explanation, Error>
pub fn default_base(git_dir: &Path, head: Option<&str>) -> Result<String, Error>
```

`root` is any directory inside the Gradle build; the nearest directory at or above it that holds
`settings.gradle(.kts)` is the build root, as Gradle resolves it.

### `SelectOptions`

| Field | Default | Meaning |
|---|---|---|
| `base` | [`default_base`](#default_base) | Commit the change is measured from |
| `head` | the work tree | Commit the change ends at; `None` includes uncommitted changes |
| `git_dir` | the build root | Repository to diff in |
| `config` | `<root>/kt-affected.toml`, else defaults | [Configuration](configuration.md) file |
| `cache_dir` | `<root>/build/kt-affected` | Where the graph cache lives |
| `static_discovery` | `false` | Ignore a cached Gradle export; static discovery only |
| `threads` | rayon's global pool | Run the call on a pool of this many threads, dropped when it returns |

Build it with the chained setters (the struct is `#[non_exhaustive]`, so a struct literal is not
available outside the crate):

```rust
let opts = SelectOptions::new().base("origin/main").head("HEAD").cache_dir("/tmp/kt-affected");
```

These are the CLI's `--base`, `--head`, `--git-dir`, `--config`, `--cache-dir` and
`--static-discovery`.

### `Selection`

Exactly what `kt-affected select --format json` prints, and serde `Serialize` + `Deserialize` with
the same camelCase names:

```json
{"base": "…", "changedFiles": ["…"], "reasons": ["…"],
 "groups": [{"module": ":core", "task": "jvmTest", "classes": ["…"], "full": false, "total": 12,
             "selected": 1, "local": true, "failOpenForks": false, "resultDir": "core/build/test-results/jvmTest"}],
 "error": null}
```

`error` is always `None` in a returned `Selection`: a failure is an `Err`. The field exists so the
type also reads the JSON the CLI prints when it cannot answer.

- `to_gradle_args() -> Vec<String>`: the local groups' task paths, each followed by `--tests <class>`
  pairs unless the whole task runs. `select --format gradle` prints them joined by spaces. An empty
  list means nothing needs to run.
- `to_classes() -> Vec<String>`: one `<module>:<task>\t<class>` line per selected class, every group.
  `select --format classes` prints them joined by newlines.

See [CI](ci.md#output-formats) for what `full` and `local` mean.

### `Explanation`

`explain` returns the same selection plus why:

- `selection: Selection`
- `verdicts: Vec<Verdict>`: per changed declaration, `{file, declaration, line, functional, reason}`.
  A cosmetic (non-functional) change seeds nothing.
- `paths: Vec<TestPath>`: per test class the graph reached, `{test, steps: [{declaration, how}]}`, from
  the test back to the seed: the last step is the changed declaration, and each step's `how` says how
  it was reached from the step after it. A class selected by a path, scanner or smoke rule has no
  path.

`render()` (or `Display`) gives the text `kt-affected explain` prints. All of it is serde
serialisable.

### `default_base`

The base `select` uses when `base` is `None`: the merge-base of `head` (HEAD when `None`) with the
remote's default branch, `origin/HEAD`, else `origin/main`, else `origin/master`.

## Errors

A selector that cannot answer must never be read as "no tests". Every such case is an `Err`, never an
empty `Selection`; treat any `Err` as "run everything".

| Variant | When |
|---|---|
| `NoBuildRoot(dir)` | no `settings.gradle(.kts)` at or above `root` |
| `Git(msg)` | a git command failed (unknown ref, not a repository), or no base was given and none could be resolved |
| `Config(msg)` | `kt-affected.toml` could not be read or parsed |
| `Graph(msg)` | the graph cache could not be built, updated or opened |
| `CouldNotAnswer(msg)` | the selector cannot tell which tests the change reaches |

`Display` is the message alone, the same text the CLI prints. `Error` is `#[non_exhaustive]`; match
with a wildcard arm.

The CLI maps these to its existing behaviour: a config error, or no base to compare with, prints the
message and exits 3; any other error prints the JSON plan with `error` set and exits 3.

## Memory and threads

Nothing stays resident between calls. Each call opens the graph cache from disk (memory-mapped),
brings it up to date incrementally exactly as `kt-affected select` does, computes, and drops it all
before it returns. There are no caches in `static`s that grow with use: the only statics are
compiled regular expressions, and `explain`'s path capture lives in the call.

What does outlive a call is per-thread and bounded by the thread count, not by the number of calls:
the parallel work runs on rayon's global pool, whose worker threads keep the regular-expression
match caches and kt-syntax's recycled parser buffers they used (at most 16 buffers per thread, none
for files of ~650 KB and up). Pass `threads` to run a call on a pool of its own instead; it is
dropped when the call returns, at the cost of starting threads each call.

Measured with `examples/rss.rs` (`cargo run --release --example rss -- <root> --base HEAD~5`), each
call in a fresh process under `/usr/bin/time -l`, median of 5, on an Apple M3 Max under heavy load
from other work (so wall times are pessimistic):

| Project | Case | Wall | Peak RSS |
|---|---|---:|---:|
| okio | cold (no cache) | 0.35 s | 41.3 MB |
| okio | warm (cache present, nothing changed) | 0.24 s | 12.0 MB |
| sqldelight | cold (no cache) | 0.68 s | 45.6 MB |
| sqldelight | warm (cache present, nothing changed) | 0.46 s | 11.8 MB |

Repeated warm calls in one process (`--calls 100`):

| Project | RSS after call 1 | after call 20 | after call 100 | live heap after call 1 / 20 / 100 |
|---|---:|---:|---:|---:|
| okio | 11.1 MB | 14.6 MB | 16.9 MB | 0.70 / 1.33 / 1.77 MB |
| sqldelight | 10.8 MB | 12.6 MB | 13.8 MB | 0.90 / 1.41 / 1.94 MB |

Live heap is counted by the example's allocator. It rises over the first calls as more of the pool's
threads warm their caches, then holds at about 2 MB and does not keep growing; RSS follows it, plus
allocator pages the process keeps for reuse. A call that parses (cold, or after edits) leaves the
pool threads' parser buffers behind as well: on okio about 11 MB of live heap after a cold call,
the same after the next calls. macOS `leaks` reports no leaked memory after cold, delta or warm
calls.

## Concurrency

Safe to call from several threads, or several processes, on one cache dir. Updates serialise: each
takes an exclusive advisory lock on `<cache_dir>/lock` from the file walk through the last rename,
so a second caller waits, then finds the cache fresh and does no work. The graph a call selects
with is the one its own update left. Readers that only map `graph.bin` take no lock; it is replaced
by rename, never written in place. Temp files are unique per writer.

The lock is the OS's (`flock` on Unix, `LockFileEx` on Windows), so it goes with a process that is
killed and never needs clearing by hand. A call waits at most 5 minutes for it, then returns
`Error::Graph`. On a file system that cannot lock at all, updates run unlocked, so there only one
writer at a time is safe. Separately, `graph.bin` records the `parses.bin` it was built with, and a
mismatched pair (a crash between the two renames, say) is rebuilt cold rather than read.

## Stability

The public API is `lib.rs` alone; every other module is private. Public structs and enums are
`#[non_exhaustive]`, so fields and variants can be added in a minor release. Before 1.0 a minor
release may also change selection results or the plan format (see the [changelog](../CHANGELOG.md)).

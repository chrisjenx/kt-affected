# Contributing

Thanks for helping. Issues with a build kt-affected misreads are the most valuable contribution:
include `kt-affected discover --json` output (or the part that is wrong) and, if you can, what
`kt-affected discover --gradle-export` reports for the same build.

## Building and checking

```bash
cargo build --release
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
scripts/oss-golden.sh target/release/kt-affected     # static discovery of pinned okio/sqldelight vs goldens
```

CI runs all of these on Linux, macOS and Windows, plus a `cargo check` on the minimum supported Rust
version (see `rust-version` in `Cargo.toml`).

## Changes that move results

- **Discovery:** if `oss-golden.sh` reports a diff you intended, run it with `--update` and commit
  the new goldens; the diff is part of the review.
- **Selection:** a rule that selects *less* must come with evidence that what it drops is safe: an
  end-to-end test in `tests/` showing the dropped test cannot observe the change, and, for a broad
  rule, numbers from real pull requests of a real build.
  Selecting more needs a test showing the case it now catches.
- **Speed:** the Benchmark workflow compares your branch with its base on the same runner. Note any
  large change in the pull request.

## Pull requests

Keep each one to one change, add a line under `## [Unreleased]` in `CHANGELOG.md` for anything a
user would notice, and describe how you tested it.

## Releasing (maintainers)

1. Move the `[Unreleased]` entries under a new `## [X.Y.Z]` heading and set `version` in `Cargo.toml`.
2. Merge, then `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The Release workflow builds every target and publishes the GitHub release with that CHANGELOG section.

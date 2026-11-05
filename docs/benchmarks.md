# Benchmarks

## Method

- The release binary is built first and never timed.
- Every run is a fresh process; wall clock and peak RSS are recorded per run, and the median is
  reported. The OS file cache is warm.
- Cases: `cold` (cache deleted), `warm` (nothing changed), `body` (a comment appended to one file),
  `decl` (a new top-level declaration). The edited file is restored byte for byte with its mtime,
  then one unmeasured run re-settles the cache.
- Discovery is static; the measured project's Gradle is never run.

The README's headline numbers were measured this way on a 14k-file production KMP app, beside the
selector kt-affected replaced there and beside Gradle itself.

## Open-source projects (CI)

`.github/workflows/bench.yml` runs `bench/oss_bench.py` on pinned okio and sqldelight checkouts on
every push and pull request. On a pull request the base and head binaries run interleaved on the
same runner and the job summary shows the change per case. Shared runners are noisy, so it reports
and never fails.

## Your own project

```bash
cargo build --release
python3 bench/oss_bench.py --root ~/src/some-kmp-project --bin target/release/kt-affected
python3 bench/oss_bench.py --root ... --bin target/release/kt-affected --bin-base /tmp/kt-affected-main  # A/B two builds
```

Numbers from a large public build are a welcome contribution.

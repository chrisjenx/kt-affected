//! Checks and previews used by the benchmark harness: incremental == cold, and a reachability preview.

use crate::cache::{self, Packed};
use crate::discover::Build;
use crate::parse::{FLAG_OVERRIDE, FLAG_TEST_ROOT};
use std::collections::{HashSet, VecDeque};
use std::path::Path;

/// Rebuilds cold into `scratch` and compares every edge with the graph in `dir`.
pub fn verify(
    root: &Path,
    build: &Build,
    dir: &Path,
    scratch: &Path,
) -> std::io::Result<(usize, usize, usize)> {
    let _ = std::fs::remove_dir_all(scratch);
    cache::update(root, build, scratch, true)?;
    let a =
        Packed::open(&dir.join("graph.bin")).ok_or_else(|| std::io::Error::other("no graph"))?;
    let b = Packed::open(&scratch.join("graph.bin"))
        .ok_or_else(|| std::io::Error::other("no cold graph"))?;
    let ea = cache::edge_set(&a);
    let eb = cache::edge_set(&b);
    let sa: HashSet<_> = ea.iter().collect();
    let sb: HashSet<_> = eb.iter().collect();
    Ok((
        sa.intersection(&sb).count(),
        sa.difference(&sb).count(),
        sb.difference(&sa).count(),
    ))
}

/// Test-class roots reachable from every declaration of `changed` files (reverse edges only,
/// plus the members an overriding seed overrides — the spike's preview, not the real selector).
pub fn preview(p: &Packed, changed: &[String], virtual_dispatch: bool) -> (usize, usize) {
    let mut seeds = Vec::new();
    for f in 0..p.n_files() {
        if changed.iter().any(|c| c == p.file_path(f)) {
            let base = p.file_base(f) as usize;
            let next = if f + 1 < p.n_files() {
                p.file_base(f + 1) as usize
            } else {
                p.n_nodes()
            };
            seeds.extend(base as u32..next as u32);
        }
    }
    let mut seen: HashSet<u32> = seeds.iter().copied().collect();
    let mut q: VecDeque<u32> = seeds.iter().copied().collect();
    if virtual_dispatch {
        // Overridden members: same-named members of any type the seed's owner (transitively) extends.
        // Supertypes are the edges whose source is a type and whose target is a type it names in its header;
        // the preview approximates them with "referenced_by from the owner".
        for &s in &seeds {
            let row = p.node(s as usize);
            let flags = (row[1] >> 8) as u8;
            if flags & FLAG_OVERRIDE != 0 && (row[2] as i32) >= 0 {
                let name = row[0];
                for t in 0..p.n_nodes() as u32 {
                    let tr = p.node(t as usize);
                    if tr[0] == name && (tr[2] as i32) >= 0 && seen.insert(t) {
                        q.push_back(t);
                    }
                }
            }
        }
    }
    while let Some(n) = q.pop_front() {
        for &r in p.referenced_by(n as usize) {
            if seen.insert(r) {
                q.push_back(r);
            }
        }
    }
    let roots: HashSet<u32> = seen
        .iter()
        .map(|&n| {
            let mut m = n;
            while (p.node(m as usize)[2] as i32) >= 0 {
                m = p.node(m as usize)[2];
            }
            m
        })
        .filter(|&r| (p.node(r as usize)[1] >> 8) as u8 & FLAG_TEST_ROOT != 0)
        .collect();
    (seeds.len(), roots.len())
}

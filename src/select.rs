//! `kt-affected select`: the test classes a diff could affect, grouped by Gradle task.
//!
//! - a test source set runs under every test task whose compilation includes it (discovery's
//!   dependsOn closure), not one task per source-set name;
//! - a member change reaches referrers of its container that hold a class literal of it
//!   (`X::class`, `KClass<X>`), which reflect over its members without naming them;
//! - an ignored or doc path a test names by string is a mention, not dropped.

use crate::cache::Packed;
use crate::change;
use crate::config::{CONFIG_FILE, Config};
use crate::discover::{Build, KMP_TARGETS, is_test_source_set};
use crate::git::{Hunk, Repo};
use crate::route::{self, Ctx, Module, Modules, Routing};
use crate::seeds::{self, Routes, View};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

pub const UNMAPPED_TASK: &str = "*";

/// One Gradle test task to run: the whole task, or the listed classes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Group {
    /// Gradle project path (`:core`, `:` for the root project).
    pub module: String,
    /// Test task name in that project (`jvmTest`, `test`), or `*` when the change maps to no task.
    pub task: String,
    /// Fully qualified test classes to run; every class of the task when `full`.
    pub classes: Vec<String>,
    /// Run the whole task, not only `classes`.
    pub full: bool,
    /// Test classes the task holds.
    pub total: usize,
    /// Length of `classes`.
    pub selected: usize,
    /// A local loop runs this task (`[tasks] local`); runners defer the others.
    pub local: bool,
    /// The module is listed in `[modules] fail_open_forks`: a whole run of it needs parallel forks.
    pub fail_open_forks: bool,
    /// `<module dir>/build/test-results/<task>`, relative to the build root, when the module is known.
    pub result_dir: Option<String>,
}

/// What `select` computed; the public `Selection` is built from it.
#[derive(Debug)]
pub struct Plan {
    pub base: String,
    pub changed_files: Vec<String>,
    pub reasons: Vec<String>,
    pub groups: Vec<Group>,
    pub explain: Option<Explain>,
}

/// Why each changed declaration seeds or not, and how each selected test is reached.
#[derive(Debug, Default)]
pub struct Explain {
    /// (file, declaration, line, functional, reason)
    pub verdicts: Vec<(String, String, u32, bool, String)>,
    /// (test class, steps from the test back to a seed: (declaration, how it was reached))
    pub paths: Vec<(String, Vec<(String, String)>)>,
}

pub fn modules_of(
    root: &std::path::Path,
    build: &Build,
    cfg: &Config,
) -> (Modules, BTreeMap<String, String>) {
    let mut list: Vec<Module> = build
        .projects
        .iter()
        .filter(|p| !p.dir.is_empty() && p.dir != ".")
        .map(|p| Module {
            id: p.path.clone(),
            prefix: format!("{}/", p.dir.trim_end_matches('/')),
            source_dirs: p.source_dirs.clone(),
        })
        .collect();
    list.sort_by(|a, b| {
        b.prefix
            .len()
            .cmp(&a.prefix.len())
            .then(a.prefix.cmp(&b.prefix))
    });
    let ids: BTreeSet<&str> = list.iter().map(|m| m.id.as_str()).collect();
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in &build.projects {
        let set: BTreeSet<String> = p
            .deps
            .iter()
            .map(|(_, t)| t.clone())
            .filter(|t| ids.contains(t.as_str()) && *t != p.path)
            .collect();
        if !set.is_empty() {
            deps.insert(p.path.clone(), set);
        }
    }
    let mut db = BTreeSet::new();
    if let Some(marker) = &cfg.database.module_marker {
        for p in &build.projects {
            let Some(bf) = &p.build_file else { continue };
            let Ok(text) = std::fs::read_to_string(root.join(bf)) else {
                continue;
            };
            let code: String = text
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            if code.contains(marker.as_str()) {
                db.insert(p.path.clone());
            }
        }
    }
    let dirs = build
        .projects
        .iter()
        .map(|p| (p.path.clone(), p.dir.clone()))
        .collect();
    (Modules { list, deps, db }, dirs)
}

/// Conventional test task per source-set name, for a module discovery found no test task in.
const NAME_TASKS: &[(&str, &str)] = &[
    ("test", "test"),
    ("jvmTest", "jvmTest"),
    ("commonTest", "jvmTest"),
    ("androidUnitTest", "testDebugUnitTest"),
    ("androidHostTest", "testAndroidHostTest"),
    ("androidDeviceTest", "connectedAndroidTest"),
    ("androidInstrumentedTest", "connectedAndroidTest"),
    ("iosTest", "iosSimulatorArm64Test"),
    ("appleTest", "iosSimulatorArm64Test"),
    ("nativeTest", "iosSimulatorArm64Test"),
    ("jsTest", "jsBrowserTest"),
    ("wasmJsTest", "wasmJsBrowserTest"),
    ("webTest", "wasmJsBrowserTest"),
    ("nonWebTest", "jvmTest"),
    ("jvmAndAndroidTest", "jvmTest"),
];

/// Test tasks a module's source set runs under: config override, else discovery's dependsOn
/// closure. When discovery saw no compilation including the set (a convention plugin added the
/// target, or the set is an intermediate one), the conventional task for its name; an unknown
/// name runs under every test task of its module, since it feeds at least one of them.
/// Returns the tasks kept and those `[tasks] only` removed.
fn tasks_for(build: &Build, cfg: &Config, module: &str, set: &str) -> (Vec<String>, Vec<String>) {
    let found: Vec<String> = if let Some(t) = cfg.tasks.source_sets.get(set) {
        t.clone()
    } else {
        let p = build.project(module);
        let direct: Vec<String> = p
            .and_then(|p| p.test_tasks.get(set))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        let named: Vec<String> = NAME_TASKS
            .iter()
            .filter(|(s, _)| *s == set)
            .map(|(_, t)| t.to_string())
            .collect();
        if !direct.is_empty() {
            direct
        } else if !named.is_empty() {
            named
        } else {
            p.map(|p| {
                p.test_tasks
                    .values()
                    .flatten()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default()
        }
    };
    if cfg.tasks.only.is_empty() {
        (found, Vec::new())
    } else {
        found.into_iter().partition(|t| cfg.tasks.only.contains(t))
    }
}

struct Index {
    owner: FxHashMap<u32, Vec<(String, String)>>,
    full: BTreeMap<(String, String), BTreeSet<String>>,
    /// Classes from source sets that run under this task alone: the task-specific smoke candidates.
    leaf: BTreeMap<(String, String), BTreeSet<String>>,
    unknown: BTreeSet<String>,
    /// Source sets per module whose every task `[tasks] only` removed: dropped, not failed open.
    excluded: BTreeMap<String, BTreeSet<String>>,
    /// `(module, kept task)` -> tasks `[tasks] only` removed from a source set that also runs there.
    only_dropped: BTreeMap<(String, String), BTreeSet<String>>,
    /// Tasks whose build script filters the classes they run (`filter.includeTestsMatching`).
    filters: BTreeMap<(String, String), crate::discover::TestFilter>,
}

impl Index {
    /// Can test class `fq` run under the task `key`?
    fn admits(&self, key: &(String, String), fq: &str) -> bool {
        self.filters.get(key).map(|f| f.admits(fq)).unwrap_or(true)
    }
}

fn test_index(v: &View, mods: &Modules, build: &Build, cfg: &Config) -> Index {
    let mut by_file: FxHashMap<u32, Vec<&String>> = FxHashMap::default();
    for (f, fq) in v.test_class.values() {
        by_file.entry(*f).or_default().push(fq);
    }
    let mut idx = Index {
        owner: Default::default(),
        full: Default::default(),
        leaf: Default::default(),
        unknown: Default::default(),
        excluded: Default::default(),
        only_dropped: Default::default(),
        filters: Default::default(),
    };
    for p in &build.projects {
        for (task, f) in &p.test_filters {
            idx.filters
                .insert((p.path.clone(), task.clone()), f.clone());
        }
    }
    for f in 0..v.p.n_files() as u32 {
        let path = v.p.file_path(f as usize);
        let Some((m, set)) = mods.source_set_of(path) else {
            continue;
        };
        if !is_test_source_set(set) {
            continue;
        }
        let (tasks, dropped) = tasks_for(build, cfg, &m.id, set);
        if tasks.is_empty() {
            if dropped.is_empty() {
                idx.unknown.insert(m.id.clone());
            } else {
                idx.excluded
                    .entry(m.id.clone())
                    .or_default()
                    .insert(set.to_string());
            }
            continue;
        }
        if !dropped.is_empty() {
            for t in &tasks {
                idx.only_dropped
                    .entry((m.id.clone(), t.clone()))
                    .or_default()
                    .extend(dropped.iter().cloned());
            }
        }
        let classes = by_file.get(&f).cloned().unwrap_or_default();
        let mut keys = Vec::new();
        let alone = tasks.len() == 1;
        for t in tasks {
            let key = (m.id.clone(), t);
            let run: Vec<String> = classes
                .iter()
                .filter(|c| idx.admits(&key, c))
                .map(|s| s.to_string())
                .collect();
            if run.is_empty() && !classes.is_empty() {
                continue;
            }
            if alone {
                idx.leaf
                    .entry(key.clone())
                    .or_default()
                    .extend(run.iter().cloned());
            }
            idx.full.entry(key.clone()).or_default().extend(run);
            keys.push(key);
        }
        idx.owner.insert(f, keys);
    }
    idx
}

fn in_test_source_set(mods: &Modules, path: &str) -> bool {
    mods.source_set_of(path)
        .map(|(_, s)| is_test_source_set(s))
        .unwrap_or(false)
}

// ------------------------------------------------------------------ typed dispatch routes

/// The dispatch type argument of `r`: written straight on a base (`Base<E>`), or through generic
/// intermediate bases by substitution (`Foo : Mid<X>` where `Mid<E> : Base<E>` gives `X`).
fn dispatch_arg(
    v: &View,
    r: u32,
    bases: &FxHashSet<u32>,
    arg_re: &Regex,
    header_of: &dyn Fn(u32) -> Option<String>,
    depth: u8,
) -> Option<String> {
    static SUPER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b([A-Z]\w*)\s*<([^<>{}()]*)>").unwrap());
    let header = header_of(r)?;
    if let Some(m) = arg_re.captures(&header) {
        return Some(m[1].to_string());
    }
    if depth >= 4 {
        return None;
    }
    // Supertype clauses sit after the header's first top-level `:`; the declaration's own type
    // parameters come before it.
    let after = header
        .split_once(&format!("{} ", v.p.str(v.name[r as usize])))
        .map(|(_, b)| b)
        .unwrap_or(&header);
    for m in SUPER_RE.captures_iter(after) {
        let Some(name) = v.string_id(&m[1]) else {
            continue;
        };
        let args: Vec<&str> = m[2].split(',').map(str::trim).collect();
        for mid in 0..v.n as u32 {
            if v.parent[mid as usize] >= 0
                || v.name[mid as usize] != name
                || mid == r
                || !v.declares(mid).iter().any(|_| true)
            {
                continue;
            }
            if !descends_to(v, mid, bases) {
                continue;
            }
            let Some(mid_header) = header_of(mid) else {
                continue;
            };
            let params = type_params(&mid_header, &m[1]);
            let Some(inner) = dispatch_arg(v, mid, bases, arg_re, header_of, depth + 1) else {
                continue;
            };
            if let Some(i) = params.iter().position(|p| *p == inner) {
                if let Some(a) = args.get(i) {
                    return Some(a.to_string());
                }
            } else {
                return Some(inner); // the intermediate fixes the event type itself
            }
        }
    }
    None
}

fn descends_to(v: &View, n: u32, bases: &FxHashSet<u32>) -> bool {
    let mut seen: FxHashSet<u32> = [n].into_iter().collect();
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        for &s in v.declares(x) {
            if bases.contains(&s) {
                return true;
            }
            if seen.insert(s) {
                stack.push(s);
            }
        }
    }
    false
}

/// Type parameter names of `name<...>` in a declaration header (`E` from `<E : DomainEvent>`).
fn type_params(header: &str, name: &str) -> Vec<String> {
    let re = Regex::new(&format!(r"\b{}\s*<([^>]*)>", regex::escape(name))).unwrap();
    let Some(m) = re.captures(header) else {
        return Vec::new();
    };
    m[1].split(',')
        .filter_map(|p| {
            p.trim()
                .trim_start_matches("in ")
                .trim_start_matches("out ")
                .split(|c: char| c == ':' || c.is_whitespace())
                .next()
        })
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn dispatch_routes(
    v: &View,
    cfg: &Config,
    seeds: &FxHashSet<u32>,
    read: &dyn Fn(&str) -> Option<String>,
) -> (Routes, FxHashSet<u32>) {
    let mut out = Routes::default();
    let mut constructed = FxHashSet::default();
    for d in &cfg.dispatch {
        let base_ids: FxHashSet<u32> = d.bases.iter().filter_map(|b| v.string_id(b)).collect();
        let bases: FxHashSet<u32> = (0..v.n as u32)
            .filter(|&n| v.parent[n as usize] < 0 && base_ids.contains(&v.name[n as usize]))
            .collect();
        if bases.is_empty() {
            continue;
        }
        let alts = d
            .bases
            .iter()
            .map(|b| regex::escape(b))
            .collect::<Vec<_>>()
            .join("|");
        let arg_re = Regex::new(&format!(r"\b(?:{alts})\s*<\s*([A-Za-z_][\w.]*)\s*>")).unwrap();
        let roots: FxHashSet<u32> = seeds.iter().map(|&n| v.root[n as usize]).collect();
        let descends = |n: u32| -> bool {
            let mut seen: FxHashSet<u32> = [n].into_iter().collect();
            let mut stack = vec![n];
            while let Some(x) = stack.pop() {
                for &s in v.declares(x) {
                    if bases.contains(&s) {
                        return true;
                    }
                    if seen.insert(s) {
                        stack.push(s);
                    }
                }
            }
            false
        };
        let candidates: Vec<u32> = roots
            .iter()
            .copied()
            .filter(|r| !bases.contains(r) && descends(*r))
            .collect();
        if candidates.is_empty() {
            out.bases.extend(bases);
            continue;
        }
        let mut reverse: Option<FxHashMap<u32, Vec<u32>>> = None;
        let header_of = |r: u32| -> Option<String> {
            let text = read(v.p.file_path(v.file[r as usize] as usize))?;
            let (lo, hi) = (v.lo[r as usize] as usize, v.hi[r as usize] as usize);
            let mut header = String::new();
            for l in text.split('\n').take(hi).skip(lo.saturating_sub(1)) {
                header.push_str(l);
                header.push('\n');
                if l.contains('{') {
                    break;
                }
            }
            Some(header)
        };
        for r in candidates {
            let Some(arg) = dispatch_arg(v, r, &bases, &arg_re, &header_of, 0) else {
                continue;
            };
            let segs: Vec<&str> = arg.split('.').collect();
            let Some(last) = v.string_id(segs[segs.len() - 1]) else {
                continue;
            };
            let mut events: Vec<u32> = Vec::new();
            for n in 0..v.n as u32 {
                if v.name[n as usize] != last {
                    continue;
                }
                let mut p = v.parent[n as usize];
                let mut ok = true;
                for seg in segs[..segs.len() - 1].iter().rev() {
                    if p < 0 || v.p.str(v.name[p as usize]) != *seg {
                        ok = false;
                        break;
                    }
                    p = v.parent[p as usize];
                }
                if ok {
                    events.push(n);
                }
            }
            if events.is_empty() {
                continue;
            }
            let rev = reverse.get_or_insert_with(|| {
                let mut m: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
                for n in 0..v.n as u32 {
                    for &s in v.declares(n) {
                        m.entry(s).or_default().push(n);
                    }
                }
                m
            });
            let mut want: FxHashSet<u32> = events.iter().copied().collect();
            let mut stack = events.clone();
            while let Some(x) = stack.pop() {
                for &sub in rev.get(&x).map(|v| v.as_slice()).unwrap_or(&[]) {
                    if want.insert(sub) {
                        stack.push(sub);
                    }
                }
            }
            out.routes.insert(r, want);
            if d.construction_smoke {
                let text = read(v.p.file_path(v.file[r as usize] as usize)).unwrap_or_default();
                let lines: Vec<&str> = text.split('\n').collect();
                for &n in seeds {
                    if v.root[n as usize] != r {
                        continue;
                    }
                    if n == r {
                        constructed.insert(r);
                        continue;
                    }
                    let first = lines
                        .get((v.lo[n as usize] as usize).saturating_sub(1))
                        .copied()
                        .unwrap_or("");
                    if v.parent[n as usize] == r as i32
                        && !first.contains("fun ")
                        && first.contains('=')
                    {
                        constructed.insert(r);
                    }
                }
            }
        }
        out.bases.extend(bases);
    }
    (out, constructed)
}

// ------------------------------------------------------------------ class literals

/// Referrers of a seeded member's containers whose own lines write `C::class` or `KClass<C>`:
/// reflection over C's members reaches the member without naming it.
fn class_literal_referrers(
    v: &View,
    seeds: &FxHashSet<u32>,
    read: &dyn Fn(&str) -> Option<String>,
) -> FxHashSet<u32> {
    let mut containers: FxHashSet<u32> = FxHashSet::default();
    for &n in seeds {
        let mut p = v.parent[n as usize];
        while p >= 0 {
            containers.insert(p as u32);
            p = v.parent[p as usize];
        }
    }
    let mut texts: FxHashMap<u32, Option<Vec<String>>> = FxHashMap::default();
    let mut out = FxHashSet::default();
    for c in containers {
        let name = regex::escape(v.p.str(v.name[c as usize]));
        let re = Regex::new(&format!(
            r"\b{name}\s*::\s*class\b|KClass\s*<\s*(?:out\s+)?{name}\s*>|typeOf\s*<\s*{name}\s*>"
        ))
        .unwrap();
        for &r in v.refby(c) {
            let f = v.file[r as usize];
            let lines = texts.entry(f).or_insert_with(|| {
                read(v.p.file_path(f as usize)).map(|t| t.split('\n').map(str::to_string).collect())
            });
            let Some(lines) = lines else { continue };
            let (lo, hi) = (v.lo[r as usize] as usize, v.hi[r as usize] as usize);
            if lines
                .iter()
                .take(hi)
                .skip(lo.saturating_sub(1))
                .any(|l| re.is_match(l))
            {
                out.insert(r);
            }
        }
    }
    out
}

fn label_of(v: &View, n: u32) -> String {
    let mut parts = Vec::new();
    let mut m = n as i32;
    while m >= 0 {
        parts.push(v.p.str(v.name[m as usize]));
        m = v.parent[m as usize];
    }
    parts.reverse();
    parts.join(".")
}

/// The first-reacher chain from test root `r` back to a seed: (declaration, how it was reached from
/// the next step), test first.
fn explain_path(
    v: &View,
    via: &FxHashMap<u32, (u32, String)>,
    all: &FxHashSet<u32>,
    seeds: &FxHashSet<u32>,
    r: u32,
) -> Vec<(String, String)> {
    // The test's own declaration nearest a seed: prefer one with a recorded reacher.
    let start = (r..v.n as u32)
        .take_while(|&n| n == r || v.root[n as usize] == r)
        .filter(|n| all.contains(n) || seeds.contains(n))
        .min_by_key(|n| (!via.contains_key(n) && !seeds.contains(n), *n))
        .unwrap_or(r);
    let mut out = Vec::new();
    let mut n = start;
    let mut seen: FxHashSet<u32> = FxHashSet::default();
    while seen.insert(n) && out.len() < 80 {
        if seeds.contains(&n) {
            out.push((label_of(v, n), "seed".into()));
            break;
        }
        match via.get(&n) {
            Some((from, how)) => {
                out.push((label_of(v, n), how.clone()));
                n = *from;
            }
            None if v.parent[n as usize] >= 0 && all.contains(&(v.parent[n as usize] as u32)) => {
                out.push((label_of(v, n), "member of a reached container".into()));
                n = v.parent[n as usize] as u32;
            }
            None => {
                let mut ss: Vec<u32> = seeds.iter().copied().collect();
                ss.sort_unstable();
                match ss
                    .into_iter()
                    .find(|&s| v.supertypes(v.root[s as usize]).contains(&n))
                {
                    Some(s) => {
                        out.push((label_of(v, n), "supertype of the edited class".into()));
                        n = s;
                    }
                    None => {
                        out.push((label_of(v, n), "reached by the container walk".into()));
                        break;
                    }
                }
            }
        }
    }
    out
}

/// Debug aid: the first-reacher chain from a seed to the first test class whose FQCN contains `want`.
fn trace_path(v: &View, reached: &seeds::Reach, node_seeds: &FxHashSet<u32>, want: &str) {
    let label = |n: u32| -> String {
        let mut parts = Vec::new();
        let mut m = n as i32;
        while m >= 0 && parts.len() < 4 {
            parts.push(v.p.str(v.name[m as usize]).to_string());
            m = v.parent[m as usize];
        }
        parts.reverse();
        let path = v.p.file_path(v.file[n as usize] as usize);
        format!(
            "{} @ {}",
            parts.join("."),
            path.rsplit('/').next().unwrap_or(path)
        )
    };
    let hit = reached
        .nodes()
        .filter(|&n| {
            v.test_class
                .get(&v.root[n as usize])
                .map(|(_, fq)| fq.contains(want))
                .unwrap_or(false)
        })
        .min();
    let Some(mut n) = hit else {
        eprintln!("trace: no reached test matches {want}");
        return;
    };
    let mut seeds_sorted: Vec<u32> = node_seeds.iter().copied().collect();
    seeds_sorted.sort_unstable();
    eprintln!(
        "trace seeds: {}",
        seeds_sorted
            .iter()
            .map(|&s| label(s))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for &s0 in &seeds_sorted {
        let mut p = s0 as i32;
        while p >= 0 {
            eprintln!(
                "trace declares {} -> [{}]",
                label(p as u32),
                v.declares(p as u32)
                    .iter()
                    .map(|&t| label(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for &t in v.declares(p as u32) {
                let d2 = v.declares(t);
                if !d2.is_empty() {
                    eprintln!(
                        "trace   declares {} -> {} items, e.g. {}",
                        label(t),
                        d2.len(),
                        d2.iter()
                            .take(4)
                            .map(|&x| label(x))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            p = v.parent[p as usize];
        }
    }
    eprintln!("trace to {}:", label(n));
    for _ in 0..60 {
        match reached.via.get(&n) {
            Some(&(from, how)) => {
                eprintln!("  <-[{how}] {}", label(from));
                n = from;
            }
            None => {
                eprintln!(
                    "  (origin{})",
                    if node_seeds.contains(&n) {
                        ": seed"
                    } else {
                        ": virtual walk"
                    }
                );
                break;
            }
        }
    }
}

// ------------------------------------------------------------------ select

pub struct Inputs<'a> {
    pub repo: &'a Repo,
    pub build: &'a Build,
    pub packed: &'a Packed,
    pub cfg: &'a Config,
    /// Fill `Plan::explain`.
    pub explain: bool,
}

pub fn select(inp: &Inputs) -> Result<Plan, String> {
    let (repo, build, cfg) = (inp.repo, inp.build, inp.cfg);
    let v = View::new(inp.packed);
    let mut explain: Option<Box<Explain>> = inp.explain.then(Default::default);
    let (mods, dirs) = modules_of(&repo.root, build, cfg);
    let idx = test_index(&v, &mods, build, cfg);
    let changed = repo.changed()?;
    let got = repo.hunks();
    let have_hunks = got.is_some();
    let mut hunks: HashMap<String, Vec<Hunk>> = got.unwrap_or_default();

    let mut reasons: Vec<String> = Vec::new();
    let mut full_modules: BTreeSet<String> = idx.unknown.clone();
    for m in &idx.unknown {
        reasons.push(format!("{m}: test source set with no test task"));
    }
    let read = |p: &str| repo.read(p);
    let read_base = |p: &str| repo.read_base(p);
    let build_logic_text: std::cell::OnceCell<Vec<String>> = std::cell::OnceCell::new();
    let build_logic = || -> &[String] {
        build_logic_text.get_or_init(|| {
            let mut paths: Vec<String> = vec![
                "build.gradle.kts".into(),
                "build.gradle".into(),
                "gradle.properties".into(),
            ];
            paths.extend(build.inputs.iter().cloned());
            for p in &build.projects {
                if let Some(b) = &p.build_file {
                    paths.push(b.clone());
                }
            }
            for inc in &build.included_builds {
                walk_kt(&repo.root, &format!("{inc}/src/main"), &mut paths);
            }
            for g in &cfg.selector.build_logic_globs {
                walk_kt(&repo.root, g, &mut paths);
            }
            paths.sort();
            paths.dedup();
            paths.iter().filter_map(|p| repo.read(p)).collect()
        })
    };
    // (included build dir with a trailing '/', supplies plugins) per included build.
    let included: std::cell::OnceCell<Vec<(String, bool)>> = std::cell::OnceCell::new();
    let included_build_of = |path: &str| -> Option<bool> {
        included
            .get_or_init(|| included_builds(&repo.root, build))
            .iter()
            .find(|(d, _)| path.starts_with(d.as_str()))
            .map(|(_, plugin)| *plugin)
    };
    let in_plugin_build = |path: &str| included_build_of(path) == Some(true);
    let ctx = Ctx {
        read: &read,
        read_base: &read_base,
        build_logic: &build_logic,
        in_plugin_build: &in_plugin_build,
    };

    let mut seed_files: BTreeSet<String> = BTreeSet::new();
    let mut classpath_readers: BTreeSet<String> = BTreeSet::new();
    let mut routing = Routing::default();
    let mut global_full = false;
    for path in &changed {
        if path == CONFIG_FILE
            || cfg
                .selector
                .self_paths
                .iter()
                .any(|s| s == path || (s.ends_with('/') && path.starts_with(s.as_str())))
        {
            reasons.push(format!("{path}: selector changed"));
            global_full = true;
            continue;
        }
        if cfg
            .selector
            .doc_suffixes
            .iter()
            .any(|s| path.ends_with(s.as_str()))
            || cfg
                .selector
                .ignored_prefixes
                .iter()
                .any(|p| path.starts_with(p.as_str()))
        {
            route::named_only(path, &mods, &mut routing);
            continue;
        }
        let Some(module) = mods.module_of(path).cloned() else {
            // An included library build is substituted for its coordinates: its code is on the
            // classpath of whoever depends on them, and the graph links it like any other.
            if path.ends_with(".kt")
                && v.path_file.contains_key(path.as_str())
                && included_build_of(path) == Some(false)
            {
                seed_files.insert(path.clone());
                continue;
            }
            if cfg
                .selector
                .outside_module_ignored_suffixes
                .iter()
                .any(|s| path.ends_with(s.as_str()))
            {
                route::named_only(path, &mods, &mut routing);
                continue;
            }
            if route::route(path, &mods, cfg, &ctx, &mut routing) {
                continue;
            }
            reasons.push(format!("{path}: outside every module"));
            global_full = true;
            continue;
        };
        let base = path.rsplit('/').next().unwrap_or(path);
        if cfg
            .selector
            .module_ignored_basenames
            .iter()
            .any(|b| b == base)
        {
            continue;
        }
        if path.ends_with(".kt") && v.path_file.contains_key(path.as_str()) {
            seed_files.insert(path.clone());
            continue;
        }
        if route::route(path, &mods, cfg, &ctx, &mut routing) {
            continue;
        }
        if cfg
            .database
            .synced_inputs
            .iter()
            .any(|s| path.starts_with(s.as_str()))
        {
            let readers: BTreeSet<String> = mods
                .db
                .iter()
                .filter(|m| **m != module.id)
                .cloned()
                .collect();
            if !readers.is_empty() {
                reasons.push(format!(
                    "{path}: on the test classpath of {}",
                    readers.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
                classpath_readers.extend(readers);
            }
        }
        if !path.ends_with(".kt") {
            reasons.push(format!("{path}: not a Kotlin source"));
        } else {
            reasons.push(format!("{path}: deleted or unindexed"));
        }
        full_modules.insert(module.id.clone());
    }
    if global_full {
        full_modules = mods.all();
    } else if !full_modules.is_empty() {
        full_modules = mods.reverse_closure(&full_modules);
    }
    full_modules.extend(classpath_readers);
    full_modules.extend(routing.full_modules.iter().cloned());
    reasons.append(&mut routing.reasons);

    // Mentions become one-line hunks on the lines that write the name.
    let mut mention_only: BTreeSet<String> = BTreeSet::new();
    // The mention hunks per file: lines that name something changed elsewhere, not edits.
    let mut mentioned: BTreeMap<String, Vec<Hunk>> = BTreeMap::new();
    if !routing.mentions.is_empty() {
        let files: Vec<(&str, Option<&str>)> = (0..v.p.n_files())
            .map(|f| {
                let path = v.p.file_path(f);
                (path, mods.module_of(path).map(|m| m.id.as_str()))
            })
            .collect();
        let read_sync = |p: &str| -> Option<String> {
            std::fs::read(repo.root.join(p))
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let extra = route::mention_hunks(&routing.mentions, &files, &read_sync);
        for (path, found) in extra {
            if seed_files.contains(&path) && (!have_hunks || !hunks.contains_key(&path)) {
                continue; // already seeded whole
            }
            if !hunks.contains_key(&path) && !seed_files.contains(&path) {
                mention_only.insert(path.clone()); // seed just the lines that name it
            }
            mentioned
                .entry(path.clone())
                .or_default()
                .extend(found.iter().cloned());
            hunks.entry(path.clone()).or_default().extend(found);
            seed_files.insert(path);
        }
    }

    // Node seeds.
    let mut node_seeds: FxHashSet<u32> = FxHashSet::default();
    let mut tests_in: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for (&r, (f, _)) in &v.test_class {
        tests_in.entry(*f).or_default().push(r);
    }
    for path in &seed_files {
        let Some(&f) = v.path_file.get(path.as_str()) else {
            continue;
        };
        let ids = v.file_nodes(f);
        let file_hunks =
            if (have_hunks || mention_only.contains(path)) && !v.p.file_error(f as usize) {
                hunks.get(path)
            } else {
                None
            };
        let hit = match file_hunks {
            None => None,
            Some(h) => {
                let (new_text, old_text) = (repo.read(path), repo.read_base(path));
                let new = new_text.as_deref().and_then(seeds::lines_of);
                let old = old_text.as_deref().and_then(seeds::lines_of);
                let hit = seeds::touched(&v, ids.clone(), h, new.as_ref(), old.as_ref());
                match hit {
                    Some(t) if cfg.changes.classify && !t.is_empty() => {
                        // A declaration a mention seeds is named by a change elsewhere: never classified.
                        let named: FxHashSet<u32> = mentioned
                            .get(path)
                            .and_then(|m| {
                                seeds::touched(&v, ids.clone(), m, new.as_ref(), old.as_ref())
                            })
                            .unwrap_or_default();
                        let edited: FxHashSet<u32> = t.difference(&named).copied().collect();
                        let (mut keep, verdicts) = change::classify(
                            &v,
                            &edited,
                            new_text.as_deref(),
                            old_text.as_deref(),
                            &cfg.changes.cosmetic_annotations,
                        );
                        keep.extend(named.iter().copied());
                        if let Some(e) = explain.as_mut() {
                            for x in verdicts {
                                let i = x.node as usize;
                                e.verdicts.push((
                                    path.clone(),
                                    label_of(&v, x.node),
                                    v.lo[i],
                                    x.functional,
                                    x.reason,
                                ));
                            }
                        }
                        Some(keep)
                    }
                    other => {
                        if let (Some(e), Some(t)) = (explain.as_mut(), other.as_ref()) {
                            let mut t: Vec<u32> = t.iter().copied().collect();
                            t.sort_unstable();
                            for n in t {
                                let why = if cfg.changes.classify {
                                    "functional"
                                } else {
                                    "classification off"
                                };
                                e.verdicts.push((
                                    path.clone(),
                                    label_of(&v, n),
                                    v.lo[n as usize],
                                    true,
                                    why.into(),
                                ));
                            }
                        }
                        other
                    }
                }
            }
        };
        if hit.is_none()
            && let Some(e) = explain.as_mut()
        {
            e.verdicts.push((path.clone(), "(whole file)".into(), 0, true, "seeded whole: no line-level hunks, or a package/file-annotation/unowned-line change".into()));
        }
        match hit {
            Some(h) => node_seeds.extend(h),
            None => node_seeds.extend(ids),
        }
        if in_test_source_set(&mods, path) {
            node_seeds.extend(tests_in.get(&f).into_iter().flatten().copied());
        }
    }
    node_seeds.extend(class_literal_referrers(&v, &node_seeds, &read));

    let mut picked: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut constructed_modules: BTreeSet<String> = BTreeSet::new();
    if !node_seeds.is_empty() {
        let (mut routes, constructed) = dispatch_routes(&v, cfg, &node_seeds, &read);
        routes.dispatch_homes = seeds::di_bound(
            &v,
            &cfg.di.binding_annotations,
            &cfg.di.provider_annotations,
        );
        routes.follow_dispatch =
            !(cfg.di.binding_annotations.is_empty() && cfg.di.provider_annotations.is_empty());
        routes.dispatch_rounds = cfg.di.dispatch_rounds;
        routes.typed_dispatch = cfg.di.dispatch == "typed";
        routes.trace = inp.explain;
        if routes.follow_dispatch {
            let text_of = |p: &str| repo.read(p);
            routes.di_graphs = std::rc::Rc::new(seeds::DiGraphs::new(
                &v,
                &cfg.container_gate.graph_annotations,
                &text_of,
            ));
        }
        if cfg.selector.classpath_bound {
            let edited: BTreeSet<String> = node_seeds
                .iter()
                .filter_map(|&n| {
                    mods.module_of(v.p.file_path(v.file[n as usize] as usize))
                        .map(|m| m.id.clone())
                })
                .collect();
            let seeds_known = node_seeds.iter().all(|&n| {
                mods.module_of(v.p.file_path(v.file[n as usize] as usize))
                    .is_some()
            });
            if seeds_known && !edited.is_empty() {
                let can = mods.reverse_closure(&edited);
                routes.classpath_ok = std::rc::Rc::new(
                    (0..v.p.n_files())
                        .map(|f| {
                            mods.module_of(v.p.file_path(f))
                                .map(|m| can.contains(&m.id))
                                .unwrap_or(true)
                        })
                        .collect(),
                );
            }
        }
        // The ungated walk, kept to bound the gated one: the gate only ever drops what this reaches.
        let wide_roots: Option<FxHashSet<u32>> = cfg.container_gate.enabled.then(|| {
            let reached = seeds::symbol_closure(&v, &routes, &node_seeds, true, true);
            let extra = seeds::unnamed_receiver_reach(&v, &routes, &node_seeds, &reached);
            reached
                .nodes()
                .chain(extra)
                .map(|n| v.root[n as usize])
                .collect()
        });
        let mut runs_names: FxHashSet<u32> = FxHashSet::default();
        if cfg.container_gate.enabled {
            (routes.gate, routes.ungated) = seeds::container_gate(
                &v,
                &cfg.container_gate.graph_annotations,
                &cfg.container_gate.graph_types,
                &node_seeds,
                &runs_names,
            );
        }
        for n in constructed {
            if let Some(m) = mods.module_of(v.p.file_path(v.file[n as usize] as usize)) {
                constructed_modules.insert(m.id.clone());
            }
        }
        let mut reached = seeds::symbol_closure(&v, &routes, &node_seeds, true, true);
        // The gate admits callers of any member of an edited class that runs the edit: grow those
        // names until the closure stops adding any (bounded; past it the walk runs ungated).
        if routes.gate.is_some() {
            let seed_roots: FxHashSet<u32> =
                node_seeds.iter().map(|&n| v.root[n as usize]).collect();
            let mut rounds = 0;
            loop {
                let grown: FxHashSet<u32> = reached
                    .runs
                    .iter()
                    .filter(|&&n| {
                        v.parent[n as usize] >= 0 && seed_roots.contains(&v.root[n as usize])
                    })
                    .map(|&n| v.name[n as usize])
                    .collect();
                if grown.is_subset(&runs_names) {
                    break;
                }
                runs_names.extend(grown);
                rounds += 1;
                if rounds > 8 {
                    routes.gate = None;
                } else {
                    (routes.gate, routes.ungated) = seeds::container_gate(
                        &v,
                        &cfg.container_gate.graph_annotations,
                        &cfg.container_gate.graph_types,
                        &node_seeds,
                        &runs_names,
                    );
                }
                reached = seeds::symbol_closure(&v, &routes, &node_seeds, true, true);
                if routes.gate.is_none() {
                    break;
                }
            }
        }
        if let Some(want) = std::env::var_os("KT_AFFECTED_TRACE") {
            trace_path(&v, &reached, &node_seeds, &want.to_string_lossy());
        }
        if explain.is_some() {
            *routes.explain_via.borrow_mut() = Some(
                reached
                    .via
                    .iter()
                    .map(|(&n, &(f, h))| (n, (f, h.to_string())))
                    .collect(),
            );
        }
        let extra = seeds::unnamed_receiver_reach(&v, &routes, &node_seeds, &reached);
        let via = routes.explain_via.borrow_mut().take();
        let mut roots: FxHashSet<u32> = reached
            .nodes()
            .chain(extra.iter().copied())
            .map(|n| v.root[n as usize])
            .collect();
        if let Some(wide) = &wide_roots {
            roots.retain(|r| wide.contains(r));
        }
        // A test whose graphs all leave out every edited implementation can only hold a fake of
        // it: none of the edit runs there (`DiGraphs::never_holds`).
        let edited_roots: FxHashSet<u32> = node_seeds.iter().map(|&n| v.root[n as usize]).collect();
        if routes.follow_dispatch && edited_roots.iter().all(|&i| routes.di_graphs.excludable(i)) {
            roots.retain(|&r| {
                !v.test_class.contains_key(&r)
                    || !edited_roots
                        .iter()
                        .all(|&i| routes.di_graphs.never_holds(&v, r, i))
            });
        }
        if !routes.classpath_ok.is_empty() {
            roots.retain(|&r| {
                !v.test_class.contains_key(&r) || routes.classpath_ok[v.file[r as usize] as usize]
            });
        }
        if let (Some(e), Some(via)) = (explain.as_mut(), via) {
            let all: FxHashSet<u32> = reached.nodes().chain(extra.iter().copied()).collect();
            let mut tests: Vec<(&String, u32)> = roots
                .iter()
                .filter_map(|r| v.test_class.get(r).map(|(_, fq)| (fq, *r)))
                .collect();
            tests.sort();
            for (fq, r) in tests {
                e.paths
                    .push((fq.clone(), explain_path(&v, &via, &all, &node_seeds, r)));
            }
        }
        let platform = platform_seeds(&v, &mods, build, &node_seeds);
        for r in roots {
            let Some((f, fq)) = v.test_class.get(&r) else {
                continue;
            };
            for key in idx.owner.get(f).into_iter().flatten() {
                if !idx.admits(key, fq) {
                    continue;
                }
                if let Some(seeds) = &platform
                    && !compiles_any(build, &key.0, &key.1, seeds)
                {
                    continue;
                }
                picked.entry(key.clone()).or_default().insert(fq.clone());
            }
        }
    }
    if !constructed_modules.is_empty() && !global_full {
        routing
            .db_smoke_modules
            .extend(mods.reverse_closure(&constructed_modules));
        reasons.push(format!(
            "dispatch subclass construction changed in {}: one database-backed class per module in reach",
            constructed_modules.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if !global_full {
        let class_path: BTreeMap<&str, (&str, u32)> = v
            .test_class
            .iter()
            .map(|(&r, (f, fq))| {
                (
                    fq.as_str(),
                    (
                        v.p.file_path(*f as usize),
                        v.hi[r as usize] - v.lo[r as usize],
                    ),
                )
            })
            .collect();
        for (modules, markers) in [
            (
                routing
                    .smoke_modules
                    .difference(&full_modules)
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                Vec::new(),
            ),
            (
                routing
                    .db_smoke_modules
                    .difference(&full_modules)
                    .cloned()
                    .collect(),
                cfg.database.test_markers.clone(),
            ),
        ] {
            for (key, classes) in &idx.full {
                if modules.contains(&key.0) && picked.get(key).map(|s| s.is_empty()).unwrap_or(true)
                {
                    let pool = idx
                        .leaf
                        .get(key)
                        .filter(|l| !l.is_empty())
                        .unwrap_or(classes);
                    if let Some(c) = route::smoke_class(pool, &class_path, &read, &markers)
                        .or_else(|| route::smoke_class(classes, &class_path, &read, &markers))
                    {
                        picked.entry(key.clone()).or_default().insert(c.clone());
                    }
                }
            }
        }
        for sc in &cfg.source_scanners {
            let re = Regex::new(&sc.changed_path).map_err(|e| e.to_string())?;
            if changed.iter().any(|p| re.is_match(p)) {
                let pfx = format!("{}.", sc.test_package);
                for (&r, (f, fq)) in &v.test_class {
                    let pkg = v.p.file_package(*f as usize);
                    let _ = r;
                    if pkg == sc.test_package || pkg.starts_with(&pfx) {
                        for key in idx.owner.get(f).into_iter().flatten() {
                            if idx.admits(key, fq) {
                                picked.entry(key.clone()).or_default().insert(fq.clone());
                            }
                        }
                    }
                }
            }
        }
        let scanners = &cfg.selector.source_scanner_imports;
        if !scanners.is_empty()
            && changed
                .iter()
                .any(|p| p.ends_with(".kt") || p.ends_with(".kts"))
        {
            for (file, classes) in scanning_tests(&v, scanners, &read) {
                reasons.push(format!(
                    "{file}: scans source files — runs on any Kotlin source change"
                ));
                for (f, fq) in classes {
                    for key in idx.owner.get(&f).into_iter().flatten() {
                        if idx.admits(key, &fq) {
                            picked.entry(key.clone()).or_default().insert(fq.clone());
                        }
                    }
                }
            }
        }
    }

    let mut keys: BTreeSet<(String, String)> = picked.keys().cloned().collect();
    keys.extend(
        idx.full
            .keys()
            .filter(|k| full_modules.contains(&k.0))
            .cloned(),
    );
    let mut groups: Vec<Group> = Vec::new();
    for key in keys {
        let total = idx.full.get(&key).map(|s| s.len()).unwrap_or(0);
        let (classes, full) = if full_modules.contains(&key.0) {
            (idx.full.get(&key).cloned().unwrap_or_default(), true)
        } else {
            (picked.get(&key).cloned().unwrap_or_default(), false)
        };
        groups.push(group(
            &key.0,
            &key.1,
            classes.into_iter().collect(),
            total,
            full,
            cfg,
            &dirs,
        ));
    }
    for m in &idx.unknown {
        groups.push(group(m, UNMAPPED_TASK, vec![], 0, true, cfg, &dirs));
    }
    groups.retain(|g| !g.classes.is_empty() || g.full);

    // Module policy: manual never runs; direct-only runs only for a change under its directory.
    let mut kept = Vec::new();
    let mut omitted: BTreeMap<String, String> = BTreeMap::new();
    for g in groups {
        if cfg.modules.manual.contains(&g.module) {
            omitted.insert(
                g.module.clone(),
                format!(
                    "{}: omitted from affectedTests; run its tests explicitly",
                    g.module
                ),
            );
            continue;
        }
        let prefix = mods
            .list
            .iter()
            .find(|m| m.id == g.module)
            .map(|m| m.prefix.clone());
        let direct = prefix
            .as_ref()
            .map(|p| changed.iter().any(|c| c.starts_with(p.as_str())))
            .unwrap_or(false);
        if !cfg.modules.direct_only.contains(&g.module) || prefix.is_none() || direct {
            kept.push(g);
        } else {
            omitted.insert(
                g.module.clone(),
                format!(
                    "{}: omitted from affectedTests because it runs only for direct changes under {}",
                    g.module,
                    prefix.unwrap_or_default()
                ),
            );
        }
    }
    reasons.extend(omitted.into_values());
    reasons.extend(only_reasons(&idx, &mods, &changed, &kept));
    Ok(Plan {
        base: repo.base.clone(),
        changed_files: changed,
        reasons,
        groups: kept,
        explain: explain.map(|e| *e),
    })
}

/// Test files importing a source-scanning library (`source_scanner_imports`), with their classes.
/// Only read when some build script or catalog names the library's group, so a build without it
/// pays nothing.
fn scanning_tests(
    v: &View,
    scanners: &[String],
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, Vec<(u32, String)>)> {
    let mut by_file: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (f, fq) in v.test_class.values() {
        by_file.entry(*f).or_default().push(fq.clone());
    }
    let mut out = Vec::new();
    for (f, mut classes) in by_file {
        let path = v.p.file_path(f as usize);
        let Some(text) = read(path) else { continue };
        if !scanners.iter().any(|s| text.contains(s.as_str())) {
            continue;
        }
        let imports = text.lines().any(|l| {
            l.trim_start().strip_prefix("import ").is_some_and(|rest| {
                let rest = rest.trim_start();
                scanners.iter().any(|s| {
                    rest.strip_prefix(s.as_str())
                        .is_some_and(|r| r.is_empty() || r.starts_with(['.', ' ', ';']))
                })
            })
        });
        if imports {
            classes.sort();
            out.push((
                path.to_string(),
                classes.into_iter().map(|c| (f, c)).collect(),
            ));
        }
    }
    out
}

/// What `[tasks] only` left out, for modules the plan touches: a source set none of whose tasks it
/// kept (for a module a change lies in or a group runs in), and each task it removed beside a
/// selected group's.
fn only_reasons(idx: &Index, mods: &Modules, changed: &[String], kept: &[Group]) -> Vec<String> {
    let mut out = Vec::new();
    for (module, sets) in &idx.excluded {
        let touched = kept.iter().any(|g| &g.module == module)
            || changed.iter().any(|c| {
                mods.source_set_of(c)
                    .map(|(m, _)| &m.id == module)
                    .unwrap_or(false)
            });
        if touched {
            for set in sets {
                out.push(format!(
                    "{module}: {set} not run (excluded by [tasks] only)"
                ));
            }
        }
    }
    let mut dropped: BTreeSet<(&str, &str)> = BTreeSet::new();
    for g in kept {
        for t in idx
            .only_dropped
            .get(&(g.module.clone(), g.task.clone()))
            .into_iter()
            .flatten()
        {
            dropped.insert((g.module.as_str(), t.as_str()));
        }
    }
    for (module, task) in dropped {
        out.push(format!(
            "{module}: {task} not run (excluded by [tasks] only)"
        ));
    }
    out
}

fn group(
    module: &str,
    task: &str,
    classes: Vec<String>,
    total: usize,
    full: bool,
    cfg: &Config,
    dirs: &BTreeMap<String, String>,
) -> Group {
    Group {
        module: module.into(),
        task: task.into(),
        selected: classes.len(),
        classes,
        full,
        total,
        local: task != UNMAPPED_TASK && cfg.is_local_task(task),
        fail_open_forks: cfg.modules.fail_open_forks.contains(module),
        result_dir: dirs
            .get(module)
            .map(|d| format!("{d}/build/test-results/{task}")),
    }
}

/// The `(module, target)` of every seed when each lies in one KMP target's own main source set
/// (`jvmMain`, `iosSimulatorArm64Main`, `wasmJsMain`) that no other source set depends on; such
/// code compiles into that target alone. None when any seed lies elsewhere.
fn platform_seeds(
    v: &View,
    mods: &Modules,
    build: &Build,
    seeds: &FxHashSet<u32>,
) -> Option<BTreeSet<(String, String)>> {
    let mut out = BTreeSet::new();
    let mut files: FxHashSet<u32> = FxHashSet::default();
    for &n in seeds {
        if !files.insert(v.file[n as usize]) {
            continue;
        }
        let (m, set) = mods.source_set_of(v.p.file_path(v.file[n as usize] as usize))?;
        let target = set.strip_suffix("Main")?;
        let p = build.project(&m.id)?;
        let platform = KMP_TARGETS.contains(&target)
            && !target.starts_with("android")
            && p.targets.contains(target)
            && !p.source_sets.values().any(|ss| ss.depends_on.contains(set));
        if !platform {
            return None;
        }
        out.insert((m.id.clone(), target.to_string()));
    }
    Some(out)
}

/// Can test task `task` of `module` run code from any of `seeds` (see `platform_seeds`)? Only a
/// task known to run one KMP target's compilation (`<target>Test`, `<target>BrowserTest`, ..)
/// is told no: when that target is none of the seeds' own and, for a seed in another module,
/// that module has the target too, so the task links that module's own variant of it.
fn compiles_any(
    build: &Build,
    module: &str,
    task: &str,
    seeds: &BTreeSet<(String, String)>,
) -> bool {
    let Some(p) = build.project(module) else {
        return true;
    };
    let target = p.targets.iter().find(|t| {
        KMP_TARGETS.contains(&t.as_str())
            && !t.starts_with("android")
            && task
                .strip_prefix(t.as_str())
                .is_some_and(|rest| matches!(rest, "Test" | "BrowserTest" | "NodeTest" | "D8Test"))
    });
    let Some(t) = target else {
        return true;
    };
    seeds.iter().any(|(m, own)| {
        own == t
            || (m != module
                && !build
                    .project(m)
                    .map(|q| q.targets.contains(t))
                    .unwrap_or(false))
    })
}

/// Each included build's directory (with a trailing '/', longest first) and whether it supplies
/// plugins: included under `pluginManagement`, or a build script in it applies `kotlin-dsl` /
/// `java-gradle-plugin` or declares `gradlePlugin`. Any other is a library build.
fn included_builds(root: &std::path::Path, build: &Build) -> Vec<(String, bool)> {
    let settings = ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .find_map(|f| std::fs::read_to_string(root.join(f)).ok())
        .unwrap_or_default();
    let managed = crate::discover::plugin_management_builds(&settings);
    let mut out: Vec<(String, bool)> = build
        .included_builds
        .iter()
        .filter(|d| !d.is_empty() && *d != ".")
        .map(|d| {
            let mut scripts = Vec::new();
            walk_build_scripts(root, d, &mut scripts);
            let plugin = managed.contains(d)
                || scripts.iter().any(|t| {
                    t.contains("kotlin-dsl")
                        || t.contains("java-gradle-plugin")
                        || t.contains("gradlePlugin")
                });
            (format!("{}/", d.trim_end_matches('/')), plugin)
        })
        .collect();
    out.sort_by_key(|(d, _)| std::cmp::Reverse(d.len()));
    out
}

/// The text of every build script under `rel`, outside source and output directories.
fn walk_build_scripts(root: &std::path::Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let sub = format!("{}/{}", rel.trim_end_matches('/'), name);
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                if !matches!(name.as_str(), "src" | "build" | ".gradle" | ".git") {
                    walk_build_scripts(root, &sub, out);
                }
            }
            Ok(_) if matches!(name.as_str(), "build.gradle.kts" | "build.gradle") => {
                if let Ok(t) = std::fs::read_to_string(e.path()) {
                    out.push(t);
                }
            }
            _ => {}
        }
    }
}

fn walk_kt(root: &std::path::Path, rel: &str, out: &mut Vec<String>) {
    let dir = root.join(rel);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let sub = format!("{}/{}", rel.trim_end_matches('/'), name);
        match e.file_type() {
            Ok(t) if t.is_dir() => walk_kt(root, &sub, out),
            Ok(_) if name.ends_with(".kt") || name.ends_with(".kts") => out.push(sub),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn a_platform_seed_compiles_only_into_its_own_target_tasks() {
        let project = |path: &str, targets: &[&str]| crate::discover::Project {
            path: path.into(),
            dir: path.trim_start_matches(':').into(),
            build_file: None,
            deps: Default::default(),
            source_sets: Default::default(),
            targets: targets.iter().map(|t| t.to_string()).collect(),
            test_tasks: Default::default(),
            implicit: false,
            dep_confidence: Default::default(),
            test_filters: Default::default(),
            source_dirs: Default::default(),
        };
        let build = Build {
            root_name: "r".into(),
            projects: vec![
                project(":core", &["jvm", "wasmJs", "iosSimulatorArm64"]),
                project(":app", &["jvm", "wasmJs", "js"]),
                project(":desktop", &["desktop"]),
            ],
            included_builds: vec![],
            inputs: vec![],
            warnings: vec![],
        };
        let jvm: BTreeSet<(String, String)> = [(":core".into(), "jvm".into())].into();
        assert!(compiles_any(&build, ":core", "jvmTest", &jvm));
        assert!(!compiles_any(&build, ":core", "wasmJsBrowserTest", &jvm));
        assert!(!compiles_any(
            &build,
            ":core",
            "iosSimulatorArm64Test",
            &jvm
        ));
        assert!(
            !compiles_any(&build, ":app", "wasmJsBrowserTest", &jvm),
            "app links core's wasm variant"
        );
        assert!(
            compiles_any(&build, ":app", "jsBrowserTest", &jvm),
            "core has no js variant: can't tell"
        );
        assert!(
            compiles_any(&build, ":core", "screenshots", &jvm),
            "a task of no known target"
        );
        assert!(
            compiles_any(&build, ":desktop", "desktopTest", &jvm),
            "a named target"
        );
    }

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(name: &str, files: &[(&str, &str)]) -> Fixture {
            let dir = std::env::temp_dir()
                .join(format!("kt-affected-select-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let f = Fixture { dir };
            for (p, t) in files {
                f.write(p, t);
            }
            f.git(&["init", "-q", "-b", "main"]);
            f.git(&["-c", "user.email=t@t", "-c", "user.name=t", "add", "-A"]);
            f.git(&[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                "base",
            ]);
            f
        }
        fn write(&self, p: &str, t: &str) {
            let full = self.dir.join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, t).unwrap();
        }
        fn git(&self, args: &[&str]) {
            let s = Command::new("git")
                .arg("-C")
                .arg(&self.dir)
                .args(args)
                .status()
                .unwrap();
            assert!(s.success(), "git {args:?}");
        }
        /// Select base=HEAD against the work tree.
        fn select(&self, cfg: &Config) -> Plan {
            let cache = self.dir.join("build/kt-affected");
            let build = crate::discover::discover(&self.dir);
            crate::cache::update(&self.dir, &build, &cache, true).unwrap();
            let p = Packed::open(&cache.join("graph.bin")).unwrap();
            let repo = Repo::new(self.dir.clone(), self.dir.clone(), "HEAD", None).unwrap();
            select(&Inputs {
                repo: &repo,
                build: &build,
                packed: &p,
                cfg,
                explain: false,
            })
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn picked(plan: &Plan) -> BTreeMap<String, BTreeSet<String>> {
        plan.groups
            .iter()
            .map(|g| {
                (
                    format!("{}:{}", g.module, g.task),
                    g.classes.iter().cloned().collect(),
                )
            })
            .collect()
    }

    fn test_class(pkg: &str, name: &str, body: &str) -> String {
        format!(
            "package {pkg}\n\nimport kotlin.test.Test\n\nclass {name} {{\n    @Test\n    fun t() {{\n        {body}\n    }}\n}}\n"
        )
    }

    const SETTINGS: &str = "rootProject.name = \"fx\"\ninclude(\":lib\")\n";
    const JVM_LIB: &str = "plugins { kotlin(\"jvm\") }\n";

    const REPO_API: &str =
        "package x\n\ninterface Repo {\n    fun insert(): Int\n    fun announce(): Int\n}\n";
    const REPO_IMPL: &str = "package x\n\nclass RepoImpl : Repo {\n    override fun insert(): Int = 1\n    override fun announce(): Int = 1\n}\n";
    const REPO_IMPL_EDITED: &str = "package x\n\nclass RepoImpl : Repo {\n    override fun insert(): Int = 1\n    override fun announce(): Int = 2\n}\n";

    fn repo_fixture(name: &str) -> Fixture {
        Fixture::new(
            name,
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                ("lib/src/main/kotlin/x/Repo.kt", REPO_API),
                ("lib/src/main/kotlin/x/RepoImpl.kt", REPO_IMPL),
                (
                    "lib/src/test/kotlin/x/InsertTest.kt",
                    &test_class(
                        "x",
                        "InsertTest",
                        "val r: Repo = RepoImpl()\n        r.insert()",
                    ),
                ),
                (
                    "lib/src/test/kotlin/x/AnnounceTest.kt",
                    &test_class(
                        "x",
                        "AnnounceTest",
                        "val r: Repo = RepoImpl()\n        r.announce()",
                    ),
                ),
            ],
        )
    }

    /// `[tasks] only` drops source sets whose every task it removed, but a source set mapped to no
    /// task to begin with still fails its module open.
    #[test]
    fn only_keeps_failing_open_a_source_set_with_no_task() {
        let f = Fixture::new(
            "only-no-task",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", ""),
                ("lib/src/main/kotlin/x/Repo.kt", REPO_API),
                (
                    "lib/src/fooTest/kotlin/x/FooTest.kt",
                    &test_class("x", "FooTest", "val r: Repo? = null"),
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Repo.kt",
            &REPO_API.replace("announce", "shout"),
        );
        let mut cfg = Config::default();
        cfg.tasks.only.insert("jvmTest".into());
        cfg.tasks.source_sets.insert("fooTest".into(), Vec::new());
        let plan = f.select(&cfg);
        assert!(
            plan.groups
                .iter()
                .any(|g| g.module == ":lib" && g.task == UNMAPPED_TASK && g.full),
            "{:?}",
            plan.reasons
        );
        assert!(
            plan.reasons
                .iter()
                .any(|r| r == ":lib: test source set with no test task")
        );
    }

    /// A body edit to one member no longer walks to every caller of its interface's other members.
    #[test]
    fn a_member_body_edit_skips_holders_that_never_call_it() {
        let f = repo_fixture("gate");
        f.write("lib/src/main/kotlin/x/RepoImpl.kt", REPO_IMPL_EDITED);
        let got = picked(&f.select(&Config::default()));
        assert_eq!(
            got.get(":lib:test"),
            Some(&["x.AnnounceTest".to_string()].into_iter().collect())
        );
        let mut wide = Config::default();
        wide.container_gate.enabled = false;
        let all: BTreeSet<String> = ["x.AnnounceTest", "x.InsertTest"]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(picked(&f.select(&wide)).get(":lib:test"), Some(&all));
    }

    /// The gate is decided per container: a new class elsewhere in the diff leaves the edit's walk gated.
    #[test]
    fn a_new_class_elsewhere_in_the_diff_keeps_the_member_edit_gated() {
        let f = repo_fixture("gate-mixed");
        f.write("lib/src/main/kotlin/x/RepoImpl.kt", REPO_IMPL_EDITED);
        f.write(
            "lib/src/main/kotlin/x/Fresh.kt",
            "package x\n\nclass Fresh {\n    fun x(): Int = 1\n}\n",
        );
        let got = picked(&f.select(&Config::default()));
        assert_eq!(
            got.get(":lib:test"),
            Some(&["x.AnnounceTest".to_string()].into_iter().collect())
        );
    }

    /// A constructor-time change in the edited class keeps that class's walk full.
    #[test]
    fn a_construction_change_keeps_its_containers_walk_full() {
        let f = repo_fixture("gate-ctor");
        f.write(
            "lib/src/main/kotlin/x/RepoImpl.kt",
            "package x\n\nclass RepoImpl : Repo {\n    private val seed = 2\n    override fun insert(): Int = 1\n    override fun announce(): Int = 2\n}\n",
        );
        let all: BTreeSet<String> = ["x.AnnounceTest", "x.InsertTest"]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(
            picked(&f.select(&Config::default())).get(":lib:test"),
            Some(&all)
        );
    }

    /// A private helper's edit reaches a test through a holder chain (`Graph.repo`, an inferred local)
    /// when the test calls a member that runs the helper; a sibling test calling another member does not.
    #[test]
    fn a_holder_chain_reaches_callers_of_a_member_that_runs_the_edit() {
        let impl_src = |n: u8| {
            format!(
                "package x\n\nclass RepoImpl : Repo {{\n    override fun insert(): Int = 1\n    override fun announce(): Int = helper()\n    private fun helper(): Int = {n}\n}}\n"
            )
        };
        let f = Fixture::new(
            "gate-holders",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                ("lib/src/main/kotlin/x/Repo.kt", REPO_API),
                ("lib/src/main/kotlin/x/RepoImpl.kt", &impl_src(1)),
                (
                    "lib/src/main/kotlin/x/Graph.kt",
                    "package x\n\nclass Graph {\n    val repo: Repo get() = RepoImpl()\n}\n",
                ),
                (
                    "lib/src/test/kotlin/x/ViaGraphAnnounceTest.kt",
                    &test_class(
                        "x",
                        "ViaGraphAnnounceTest",
                        "val r = Graph().repo\n        r.announce()",
                    ),
                ),
                (
                    "lib/src/test/kotlin/x/ViaGraphInsertTest.kt",
                    &test_class(
                        "x",
                        "ViaGraphInsertTest",
                        "val r = Graph().repo\n        r.insert()",
                    ),
                ),
            ],
        );
        f.write("lib/src/main/kotlin/x/RepoImpl.kt", &impl_src(2));
        let got = picked(&f.select(&Config::default()))
            .get(":lib:test")
            .cloned()
            .unwrap_or_default();
        assert!(got.contains("x.ViaGraphAnnounceTest"), "{got:?}");
        assert!(!got.contains("x.ViaGraphInsertTest"), "{got:?}");
    }

    /// An interface member's edit changes the contract: every holder and implementer stays reached.
    #[test]
    fn an_interface_member_edit_keeps_the_full_walk() {
        let f = Fixture::new(
            "gate-contract",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                ("lib/src/main/kotlin/x/Repo.kt", REPO_API),
                ("lib/src/main/kotlin/x/RepoImpl.kt", REPO_IMPL),
                // Holds the interface (a mock, say) and names only the other member.
                (
                    "lib/src/test/kotlin/x/HeldTest.kt",
                    &test_class("x", "HeldTest", "val r: Repo = TODO()\n        r.insert()"),
                ),
            ],
        );
        f.write("lib/src/main/kotlin/x/Repo.kt", "package x\n\ninterface Repo {\n    fun insert(): Int\n    fun announce(flag: Boolean = false): Int\n}\n");
        f.write("lib/src/main/kotlin/x/RepoImpl.kt", "package x\n\nclass RepoImpl : Repo {\n    override fun insert(): Int = 1\n    override fun announce(flag: Boolean): Int = 1\n}\n");
        let got = picked(&f.select(&Config::default()));
        assert_eq!(
            got.get(":lib:test"),
            Some(&["x.HeldTest".to_string()].into_iter().collect())
        );
    }

    /// A change inside a DI graph (here named by config) changes its wiring: no gating.
    #[test]
    fn a_member_edit_inside_a_graph_type_keeps_the_full_walk() {
        let f = repo_fixture("gate-graph");
        f.write("lib/src/main/kotlin/x/RepoImpl.kt", REPO_IMPL_EDITED);
        let mut cfg = Config::default();
        cfg.container_gate.graph_types = vec!["RepoImpl".into()];
        let all: BTreeSet<String> = ["x.AnnounceTest", "x.InsertTest"]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(picked(&f.select(&cfg)).get(":lib:test"), Some(&all));
    }

    /// A supertype member calling the changed member on `this` (a template method) still reaches its callers.
    #[test]
    fn a_template_method_calling_the_edited_hook_is_never_gated() {
        let f = Fixture::new(
            "gate-template",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Base.kt",
                    "package x\n\nabstract class Base {\n    fun run(): Int = hook() + 1\n    abstract fun hook(): Int\n    fun other(): Int = 0\n}\n",
                ),
                (
                    "lib/src/main/kotlin/x/Impl.kt",
                    "package x\n\nclass Impl : Base() {\n    override fun hook(): Int = 1\n}\n",
                ),
                (
                    "lib/src/main/kotlin/x/Make.kt",
                    "package x\n\nfun make(): Base = Impl()\n",
                ),
                (
                    "lib/src/test/kotlin/x/RunTest.kt",
                    &test_class("x", "RunTest", "make().run()"),
                ),
                (
                    "lib/src/test/kotlin/x/OtherTest.kt",
                    &test_class("x", "OtherTest", "make().other()"),
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Impl.kt",
            "package x\n\nclass Impl : Base() {\n    override fun hook(): Int = 2\n}\n",
        );
        let mut wide = Config::default();
        wide.container_gate.enabled = false;
        let ungated = picked(&f.select(&wide))
            .get(":lib:test")
            .cloned()
            .unwrap_or_default();
        let got = picked(&f.select(&Config::default()));
        let sel = got.get(":lib:test").cloned().unwrap_or_default();
        assert!(
            sel.contains("x.RunTest"),
            "gated {sel:?} ungated {ungated:?}"
        );
    }

    const BUS: &str = "package x\n\nabstract class Handler<E : Any> {\n    abstract fun handle(e: E): Int\n}\n\nclass Bus(private val hs: List<Handler<*>>) {\n    @Suppress(\"UNCHECKED_CAST\")\n    fun send(e: Any): Int = hs.sumOf { (it as Handler<Any>).handle(e) }\n}\n\nclass FooEvent(val id: Int = 0)\n\nclass BarEvent(val id: Int = 0)\n";

    fn dispatch_fixture(name: &str, foo_handler: &str, extra: &[(&str, &str)]) -> Fixture {
        let mut files: Vec<(&str, String)> = vec![
            ("settings.gradle.kts", SETTINGS.into()),
            ("lib/build.gradle.kts", JVM_LIB.into()),
            ("lib/src/main/kotlin/x/Bus.kt", BUS.into()),
            ("lib/src/main/kotlin/x/FooHandler.kt", foo_handler.into()),
            ("lib/src/main/kotlin/x/BarHandler.kt", "package x\n\nclass BarHandler : Handler<BarEvent>() {\n    override fun handle(e: BarEvent): Int = 1\n}\n".into()),
            ("lib/src/test/kotlin/x/FooTest.kt", test_class("x", "FooTest", "Bus(listOf(FooHandler())).send(FooEvent())")),
            ("lib/src/test/kotlin/x/BarTest.kt", test_class("x", "BarTest", "Bus(listOf(BarHandler())).send(BarEvent())")),
            ("lib/src/test/kotlin/x/BusTest.kt", test_class("x", "BusTest", "Bus(emptyList()).send(Unit)")),
        ];
        files.extend(extra.iter().map(|(p, t)| (*p, t.to_string())));
        let refs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        Fixture::new(name, &refs)
    }

    fn dispatch_cfg() -> Config {
        let mut c: Config = toml::from_str("[[dispatch]]\nbases = [\"Handler\"]\n").unwrap();
        if std::env::var_os("KT_NO_GATE").is_some() {
            c.container_gate.enabled = false;
        }
        if std::env::var_os("KT_NO_DISPATCH").is_some() {
            c.dispatch.clear();
        }
        c
    }

    /// A handler of `Handler<FooEvent>` runs only for a FooEvent: its body edit skips the bus's other users.
    #[test]
    fn a_dispatch_handler_edit_reaches_what_names_its_event() {
        let edited = "package x\n\nclass FooHandler : Handler<FooEvent>() {\n    override fun handle(e: FooEvent): Int = 2\n}\n";
        let f = dispatch_fixture("dispatch", &edited.replace("= 2", "= 1"), &[]);
        f.write("lib/src/main/kotlin/x/FooHandler.kt", edited);
        let got = picked(&f.select(&dispatch_cfg()))
            .get(":lib:test")
            .cloned()
            .unwrap_or_default();
        assert!(got.contains("x.FooTest"), "{got:?}");
        assert!(!got.contains("x.BarTest"), "{got:?}");
        let wide = picked(&f.select(&Config::default()))
            .get(":lib:test")
            .cloned()
            .unwrap_or_default();
        assert!(
            wide.contains("x.BarTest"),
            "without the rule every bus user runs: {wide:?}"
        );
    }

    /// The event type survives a generic intermediate base: `FooHandler : Mid<FooEvent>` where
    /// `Mid<E> : Handler<E>` is routed by FooEvent, like a direct subclass.
    #[test]
    fn a_dispatch_handler_through_a_generic_base_is_routed_by_substitution() {
        let mid = (
            "lib/src/main/kotlin/x/Mid.kt",
            "package x\n\nabstract class Mid<E : Any>(\n    private val tag: String,\n) : Handler<E>() {\n    fun tagged(): String = tag\n}\n",
        );
        let edited = "package x\n\nclass FooHandler : Mid<FooEvent>(\"foo\") {\n    override fun handle(e: FooEvent): Int = 2\n}\n";
        let f = dispatch_fixture("dispatch-mid", &edited.replace("= 2", "= 1"), &[mid]);
        f.write("lib/src/main/kotlin/x/FooHandler.kt", edited);
        let got = picked(&f.select(&dispatch_cfg()))
            .get(":lib:test")
            .cloned()
            .unwrap_or_default();
        assert!(got.contains("x.FooTest"), "{got:?}");
        assert!(!got.contains("x.BarTest"), "{got:?}");
    }

    #[test]
    fn a_body_edit_selects_only_the_tests_that_reach_it() {
        let f = Fixture::new(
            "body",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Money.kt",
                    "package x\n\nclass Money {\n    fun add(): Int = 1\n}\n\nclass Other\n",
                ),
                (
                    "lib/src/test/kotlin/x/MoneyTest.kt",
                    &test_class("x", "MoneyTest", "Money().add()"),
                ),
                (
                    "lib/src/test/kotlin/x/OtherTest.kt",
                    &test_class("x", "OtherTest", "Other()"),
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Money.kt",
            "package x\n\nclass Money {\n    fun add(): Int = 2\n}\n\nclass Other\n",
        );
        let got = picked(&f.select(&Config::default()));
        assert_eq!(
            got.get(":lib:test"),
            Some(&["x.MoneyTest".to_string()].into_iter().collect())
        );
    }

    #[test]
    fn a_comment_only_edit_selects_nothing() {
        let f = Fixture::new(
            "comment",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Money.kt",
                    "package x\n\nclass Money {\n    fun add(): Int = 1\n}\n",
                ),
                (
                    "lib/src/test/kotlin/x/MoneyTest.kt",
                    &test_class("x", "MoneyTest", "Money().add()"),
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Money.kt",
            "package x\n\nclass Money {\n    // explains add\n    fun add(): Int = 1\n}\n",
        );
        assert!(f.select(&Config::default()).groups.is_empty());
    }

    /// A property whose span holds `=` rides on its type, getters included, so a test that only
    /// constructs the type still runs.
    #[test]
    fn a_getter_edit_reaches_tests_that_construct_its_type() {
        let f = Fixture::new(
            "getter",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Money.kt",
                    "package x\n\nclass Money {\n    val label: String get() = \"a\"\n}\n",
                ),
                (
                    "lib/src/test/kotlin/x/MoneyTest.kt",
                    &test_class("x", "MoneyTest", "Money()"),
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Money.kt",
            "package x\n\nclass Money {\n    val label: String get() = \"b\"\n}\n",
        );
        let plan = f.select(&Config::default());
        assert_eq!(
            plan.groups
                .iter()
                .flat_map(|g| g.classes.clone())
                .collect::<Vec<_>>(),
            vec!["x.MoneyTest".to_string()]
        );
    }

    #[test]
    fn a_kdoc_only_edit_selects_nothing() {
        let f = Fixture::new(
            "kdoc",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Money.kt",
                    "package x\n\nclass Money {\n    fun add(): Int = 1\n}\n",
                ),
                (
                    "lib/src/test/kotlin/x/MoneyTest.kt",
                    &test_class("x", "MoneyTest", "Money()"),
                ),
            ],
        );
        f.write("lib/src/main/kotlin/x/Money.kt", "package x\n\nclass Money {\n    /**\n     * Explains [add].\n     */\n    fun add(): Int = 1\n}\n");
        assert!(f.select(&Config::default()).groups.is_empty());
    }

    /// Gap (a): an intermediate test source set runs under every test task whose compilation
    /// includes it — here both the JVM and the browser one.
    #[test]
    fn a_shared_test_source_set_runs_under_every_task_that_includes_it() {
        let kmp = "plugins { kotlin(\"multiplatform\") }\nkotlin {\n    jvm()\n    wasmJs { browser() }\n    sourceSets {\n        val sharedUiTest by creating { dependsOn(commonTest.get()) }\n        jvmTest { dependsOn(sharedUiTest) }\n        val wasmJsTest by getting { dependsOn(sharedUiTest) }\n    }\n}\n";
        let f = Fixture::new(
            "tasks",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", kmp),
                (
                    "lib/src/commonMain/kotlin/x/Screen.kt",
                    "package x\n\nfun screen(): Int = 1\n",
                ),
                (
                    "lib/src/sharedUiTest/kotlin/x/ScreenUiTest.kt",
                    &test_class("x", "ScreenUiTest", "screen()"),
                ),
            ],
        );
        f.write(
            "lib/src/commonMain/kotlin/x/Screen.kt",
            "package x\n\nfun screen(): Int = 2\n",
        );
        let got = picked(&f.select(&Config::default()));
        for task in [":lib:jvmTest", ":lib:wasmJsBrowserTest"] {
            assert!(
                got.get(task)
                    .map(|s| s.contains("x.ScreenUiTest"))
                    .unwrap_or(false),
                "{task}: {got:?}"
            );
        }
    }

    /// Gap (b): adding a member reaches a test that reflects over its class through a class literal.
    #[test]
    fn a_new_member_reaches_a_test_reflecting_over_its_class() {
        let f = Fixture::new(
            "literal",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                (
                    "lib/src/main/kotlin/x/Repo.kt",
                    "package x\n\ninterface Repo {\n    fun insert()\n}\n",
                ),
                (
                    "lib/src/test/kotlin/x/InventoryTest.kt",
                    "package x\n\nimport kotlin.test.Test\n\nclass InventoryTest {\n    @Test\n    fun everyMethodIsListed() {\n        check(REPOS.all { it.java.methods.isNotEmpty() })\n    }\n\n    companion object {\n        val REPOS = listOf(Repo::class)\n    }\n}\n",
                ),
            ],
        );
        f.write(
            "lib/src/main/kotlin/x/Repo.kt",
            "package x\n\ninterface Repo {\n    fun insert()\n\n    fun markModified()\n}\n",
        );
        let got = picked(&f.select(&Config::default()));
        assert!(
            got.get(":lib:test")
                .map(|s| s.contains("x.InventoryTest"))
                .unwrap_or(false),
            "{got:?}"
        );
    }

    /// Gap (c): a doc file under an ignored path that a test reads by name is a mention, not dropped;
    /// one nothing names still selects nothing.
    #[test]
    fn an_ignored_file_a_test_names_is_still_seeded() {
        let f = Fixture::new(
            "named",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                ("docs/audit-register.md", "- a\n"),
                ("docs/unrelated.md", "- b\n"),
                (
                    "lib/src/test/kotlin/x/RegisterTest.kt",
                    &test_class(
                        "x",
                        "RegisterTest",
                        "check(java.io.File(\"../docs/audit-register.md\").exists())",
                    ),
                ),
                (
                    "lib/src/test/kotlin/x/OtherTest.kt",
                    &test_class("x", "OtherTest", "check(true)"),
                ),
            ],
        );
        f.write("docs/audit-register.md", "- a\n- c\n");
        let got = picked(&f.select(&Config::default()));
        assert_eq!(
            got.get(":lib:test"),
            Some(&["x.RegisterTest".to_string()].into_iter().collect()),
            "{got:?}"
        );
        f.git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qam",
            "next",
        ]);
        f.write("docs/unrelated.md", "- d\n");
        assert!(f.select(&Config::default()).groups.is_empty());
    }

    #[test]
    fn a_self_path_fails_every_module_open() {
        let f = Fixture::new(
            "self",
            &[
                ("settings.gradle.kts", SETTINGS),
                ("lib/build.gradle.kts", JVM_LIB),
                ("tools/rules.txt", "a\n"),
                (
                    "lib/src/test/kotlin/x/OtherTest.kt",
                    &test_class("x", "OtherTest", "check(true)"),
                ),
            ],
        );
        f.write("tools/rules.txt", "b\n");
        let mut cfg = Config::default();
        cfg.selector.self_paths = vec!["tools/".into()];
        let plan = f.select(&cfg);
        assert!(
            plan.groups.iter().all(|g| g.full) && !plan.groups.is_empty(),
            "{:?}",
            plan.groups
        );
    }

    #[allow(dead_code)]
    fn _unused(_: &Path) {}
}

//! What a diff seeds in the declaration graph, and how far a seed propagates.
//!
//! A hunk seeds the declarations whose own lines it touches; the supertype walk from those seeds
//! asks "who calls this?" of the types it adds, not "who else implements it?". Comments are judged
//! by kt-syntax tokens, so a comment-only hunk seeds nothing.

use crate::cache::{Family, Packed};
use crate::git::Hunk;
use crate::graph::{N_ANY_MEMBERS, N_ASKED, N_IN_TEST, N_MARKER, N_NO_WALK, N_PRIVATE};
use crate::parse::{
    FLAG_CTOR_PROP, FLAG_INITIALIZED, FLAG_TEST_ROOT, KIND_CLASS, KIND_CONSTRUCTOR,
    KIND_ENUM_ENTRY, KIND_INIT, KIND_INTERFACE, KIND_OBJECT, KIND_VAL, KIND_VAR, NONE,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Record every closure's first reachers (`Reach::via`), as `KT_AFFECTED_TRACE` does.
pub static TRACE: AtomicBool = AtomicBool::new(false);
/// While `Some`, every closure and receiver step adds its first reachers here (`explain`).
pub static EXPLAIN_VIA: Mutex<Option<FxHashMap<u32, (u32, String)>>> = Mutex::new(None);

/// `n` was first reached from `from` by `how` (kept only while `EXPLAIN_VIA` captures).
pub fn note(n: u32, from: u32, how: impl FnOnce() -> String) {
    if let Ok(mut g) = EXPLAIN_VIA.lock()
        && let Some(m) = g.as_mut()
        && n != from
    {
        m.entry(n).or_insert_with(|| (from, how()));
    }
}

fn capturing() -> bool {
    EXPLAIN_VIA.lock().map(|g| g.is_some()).unwrap_or(false)
}

const OPERATOR_NAMES: &[&str] = &[
    "getValue",
    "setValue",
    "provideDelegate",
    "invoke",
    "plus",
    "minus",
    "times",
    "div",
    "rem",
    "mod",
    "rangeTo",
    "rangeUntil",
    "contains",
    "get",
    "set",
    "iterator",
    "next",
    "hasNext",
    "compareTo",
    "equals",
    "inc",
    "dec",
    "unaryMinus",
    "unaryPlus",
    "not",
    "plusAssign",
    "minusAssign",
    "timesAssign",
    "divAssign",
    "remAssign",
    "component1",
    "component2",
    "component3",
    "component4",
    "component5",
    "component6",
    "component7",
    "component8",
    "component9",
    "component10",
];

/// Flat per-node tables read off the mapped graph once per selection.
pub struct View<'a> {
    pub p: &'a Packed,
    pub n: usize,
    pub name: Vec<u32>,
    pub kind: Vec<u8>,
    pub dflags: Vec<u8>,
    pub flags: Vec<u8>,
    pub parent: Vec<i32>,
    pub root: Vec<u32>,
    pub file: Vec<u32>,
    pub lo: Vec<u32>,
    pub hi: Vec<u32>,
    pub rides: Vec<bool>,
    /// test-class root -> (file, FQCN)
    pub test_class: FxHashMap<u32, (u32, String)>,
    pub path_file: FxHashMap<&'a str, u32>,
    implicit: FxHashSet<u32>,
    name_ids: std::cell::OnceCell<FxHashMap<&'a str, u32>>,
}

impl<'a> View<'a> {
    pub fn new(p: &'a Packed) -> View<'a> {
        let n = p.n_nodes();
        let mut v = View {
            p,
            n,
            name: Vec::with_capacity(n),
            kind: Vec::with_capacity(n),
            dflags: Vec::with_capacity(n),
            flags: Vec::with_capacity(n),
            parent: Vec::with_capacity(n),
            root: Vec::with_capacity(n),
            file: Vec::with_capacity(n),
            lo: Vec::with_capacity(n),
            hi: Vec::with_capacity(n),
            rides: Vec::with_capacity(n),
            test_class: FxHashMap::default(),
            path_file: FxHashMap::default(),
            implicit: FxHashSet::default(),
            name_ids: std::cell::OnceCell::new(),
        };
        for i in 0..n {
            let r = p.node(i);
            let parent = r[2] as i32;
            let kind = (r[1] & 0xff) as u8;
            let dflags = (r[1] >> 8 & 0xff) as u8;
            v.name.push(r[0]);
            v.kind.push(kind);
            v.dflags.push(dflags);
            v.flags.push(p.node_flags(i));
            v.parent.push(parent);
            v.root.push(if parent < 0 {
                i as u32
            } else {
                v.root[parent as usize]
            });
            v.file.push(r[3]);
            v.lo.push(r[4]);
            v.hi.push(r[5]);
            // Init blocks, secondary constructors, enum entries, constructor properties and
            // initialized properties run when their container is built: a change to them is one
            // to the container.
            let rides = parent >= 0
                && (matches!(kind, KIND_INIT | KIND_CONSTRUCTOR | KIND_ENUM_ENTRY)
                    || (matches!(kind, KIND_VAL | KIND_VAR)
                        && dflags & (FLAG_CTOR_PROP | FLAG_INITIALIZED) != 0));
            v.rides.push(rides);
            if dflags & FLAG_TEST_ROOT != 0 && parent < 0 {
                let f = r[3] as usize;
                let pkg = p.file_package(f);
                let nm = p.str(r[0]);
                let fq = if pkg.is_empty() {
                    nm.to_string()
                } else {
                    format!("{pkg}.{nm}")
                };
                v.test_class.insert(i as u32, (r[3], fq));
            }
        }
        for f in 0..p.n_files() {
            v.path_file.insert(p.file_path(f), f as u32);
        }
        let mut want: FxHashSet<&str> = OPERATOR_NAMES.iter().copied().collect();
        want.extend(["toString", "hashCode"]);
        for s in 0..p.n_strings() as u32 {
            if want.contains(p.str(s)) {
                v.implicit.insert(s);
            }
        }
        v
    }

    /// The interned id of `s`, if any declaration or reference wrote it.
    pub fn string_id(&self, s: &str) -> Option<u32> {
        let p = self.p;
        self.name_ids
            .get_or_init(|| (0..p.n_strings() as u32).map(|i| (p.str(i), i)).collect())
            .get(s)
            .copied()
    }

    pub fn file_nodes(&self, f: u32) -> std::ops::Range<u32> {
        let f = f as usize;
        let lo = self.p.file_base(f);
        let hi = if f + 1 < self.p.n_files() {
            self.p.file_base(f + 1)
        } else {
            self.n as u32
        };
        lo..hi
    }

    fn fam(&self, f: Family, n: u32) -> &'a [u32] {
        self.p.family(f, n as usize)
    }
    pub fn refby(&self, n: u32) -> &'a [u32] {
        self.fam(Family::RefBy, n)
    }
    pub fn declares(&self, n: u32) -> &'a [u32] {
        self.fam(Family::Declares, n)
    }
    fn in_builds(&self, n: u32) -> bool {
        self.kind[n as usize] == KIND_CLASS && !self.fam(Family::Provides, n).is_empty()
    }
    /// None: any reach builds it.
    fn builds(&self, n: u32) -> Option<&'a [u32]> {
        let b = self.fam(Family::Builds, n);
        if b == [NONE] { None } else { Some(b) }
    }
    fn provides(&self, n: u32) -> Vec<(u32, &'a [u32])> {
        let pr = self.fam(Family::Provides, n);
        let mut out = Vec::new();
        let mut k = 0;
        while k + 1 < pr.len() {
            let (m, len) = (pr[k], pr[k + 1] as usize);
            out.push((m, &pr[k + 2..k + 2 + len]));
            k += 2 + len;
        }
        out
    }

    /// t, its supertypes, and their inheritable members.
    pub fn supertypes(&self, t: u32) -> FxHashSet<u32> {
        let mut seen: FxHashSet<u32> = [t].into_iter().collect();
        let mut stack = vec![t];
        while let Some(x) = stack.pop() {
            for &s in self.declares(x) {
                if seen.insert(s) {
                    stack.push(s);
                }
            }
        }
        seen
    }
}

// ------------------------------------------------------------------ hunk seeding

/// Per-line facts about one file's text: which lines hold code (a non-trivia token), and the last
/// package/import line.
pub struct Lines {
    pub code: Vec<bool>,
    pub header: u32,
}

pub fn lines_of(text: &str) -> Option<Lines> {
    let file = kt_syntax::parse(text).ok()?;
    let n_lines = text.split('\n').count();
    let mut code = vec![false; n_lines + 2];
    let line = |off: usize| file.line_col(off).map(|l| l.line as usize + 1).unwrap_or(0);
    // A KDoc is a DOC_COMMENT node whose KDOC_* tokens are not trivia themselves.
    let docs: Vec<_> = file
        .root()
        .find_all(&[kt_syntax::SyntaxKind::DOC_COMMENT])
        .map(|d| d.range())
        .collect();
    let mut d = 0;
    for tok in file.root().tokens() {
        if tok.is_trivia() {
            continue;
        }
        let start = tok.range().start;
        while d < docs.len() && docs[d].end <= start {
            d += 1;
        }
        if d < docs.len() && docs[d].start <= start {
            continue;
        }
        let r = tok.range();
        if r.start == r.end {
            continue;
        }
        let (a, b) = (line(r.start), line(r.end.saturating_sub(1)));
        for l in a..=b.min(n_lines) {
            code[l] = true;
        }
    }
    let mut header = 0;
    let root = file.root();
    for k in [
        kt_syntax::SyntaxKind::PACKAGE_DIRECTIVE,
        kt_syntax::SyntaxKind::IMPORT_DIRECTIVE,
    ] {
        for d in root.find_all(&[k]) {
            let r = d.range();
            if r.end > r.start {
                header = header.max(line(r.end - 1) as u32);
            }
        }
    }
    Some(Lines { code, header })
}

/// Declarations whose own lines a hunk touches, or None for "the whole file".
/// `new` is the file's current text facts, `old` the base's (None: every removed line is code).
pub fn touched(
    v: &View,
    ids: std::ops::Range<u32>,
    hunks: &[Hunk],
    new: Option<&Lines>,
    old: Option<&Lines>,
) -> Option<FxHashSet<u32>> {
    let header = new.map(|l| l.header).unwrap_or(0);
    let spans: Vec<(u32, u32, u32)> = ids
        .clone()
        .map(|n| (n, v.lo[n as usize], v.hi[n as usize]))
        .collect();
    let mut kids: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
    for &(n, lo, hi) in &spans {
        if v.parent[n as usize] >= 0 {
            kids.entry(v.parent[n as usize] as u32)
                .or_default()
                .push((lo, hi));
        }
    }
    for k in kids.values_mut() {
        k.sort();
    }
    let mut roots: Vec<(u32, u32)> = spans
        .iter()
        .filter(|s| v.parent[s.0 as usize] < 0)
        .map(|s| (s.1, s.2))
        .collect();
    roots.sort();
    let owners = |a: u32, b: u32| -> Vec<u32> {
        let mut found = Vec::new();
        for &(n, lo, hi) in &spans {
            let (x, y) = (lo.max(a), hi.min(b));
            if x > y {
                continue;
            }
            if x == lo {
                found.push(n);
                continue;
            }
            let mut cur = x;
            for &(klo, khi) in kids.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
                if khi < cur {
                    continue;
                }
                if klo > cur {
                    break;
                }
                cur = khi + 1;
                if cur > y {
                    break;
                }
            }
            if cur <= y {
                found.push(n);
            }
        }
        found
    };
    let unowned = |a: u32, b: u32| -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = a;
        for &(lo, hi) in &roots {
            if hi < cur {
                continue;
            }
            out.extend(cur..lo.min(b + 1));
            cur = cur.max(hi + 1);
            if cur > b {
                break;
            }
        }
        if cur <= b {
            out.extend(cur..=b);
        }
        out
    };
    let is_code = |l: Option<&Lines>, line: u32| -> bool {
        l.map(|l| l.code.get(line as usize).copied().unwrap_or(true))
            .unwrap_or(true)
    };
    let mut sorted: Vec<&Hunk> = hunks.iter().collect();
    sorted.sort_by_key(|h| h.start);
    let mut out: FxHashSet<u32> = FxHashSet::default();
    for h in sorted {
        let added_code: Vec<bool> = (0..h.added.len() as u32)
            .map(|k| is_code(new, h.start + k))
            .collect();
        let removed_code: Vec<bool> = (0..h.removed.len() as u32)
            .map(|k| is_code(old, h.old_start + k))
            .collect();
        if !added_code.iter().any(|&c| c) && !removed_code.iter().any(|&c| c) {
            continue;
        }
        let code: Vec<&String> = h
            .added
            .iter()
            .zip(&added_code)
            .filter(|(_, c)| **c)
            .map(|(t, _)| t)
            .chain(
                h.removed
                    .iter()
                    .zip(&removed_code)
                    .filter(|(_, c)| **c)
                    .map(|(t, _)| t),
            )
            .collect();
        if code.iter().any(|t| {
            let s = t.trim_start();
            s.starts_with("package ") || s.starts_with("@file:")
        }) {
            return None;
        }
        if h.start <= header {
            if !code
                .iter()
                .all(|t| t.trim().is_empty() || t.trim().starts_with("import "))
            {
                return None;
            }
            continue;
        }
        let mut found: Vec<u32> = Vec::new();
        let mut k = 0usize;
        while k < added_code.len() {
            if !added_code[k] {
                k += 1;
                continue;
            }
            let mut j = k;
            while j + 1 < added_code.len() && added_code[j + 1] {
                j += 1;
            }
            found.extend(owners(h.start + k as u32, h.start + j as u32));
            for line in unowned(h.start + k as u32, h.start + j as u32) {
                if !h.added[(line - h.start) as usize].trim().is_empty() {
                    return None;
                }
            }
            k = j + 1;
        }
        if !found.is_empty() || !removed_code.iter().any(|&c| c) {
            out.extend(found);
            continue;
        }
        let before = if h.count > 0 { h.start - 1 } else { h.start };
        let after = if h.count > 0 {
            h.start + h.count
        } else {
            h.start + 1
        };
        let mut edge = owners(before, before);
        edge.extend(owners(after, after));
        if edge.is_empty() {
            return None;
        }
        out.extend(edge);
    }
    for n in out.clone() {
        let mut n = n;
        while v.rides[n as usize] && v.parent[n as usize] >= 0 {
            n = v.parent[n as usize] as u32;
            out.insert(n);
        }
    }
    Some(out)
}

// ------------------------------------------------------------------ closure

/// Handler roots routed by their dispatch type argument: root -> event declarations (and subtypes),
/// plus the base roots the routed walks must not enter.
#[derive(Default)]
pub struct Routes {
    pub routes: FxHashMap<u32, FxHashSet<u32>>,
    pub bases: FxHashSet<u32>,
    /// Declarations that write a changed member's name, gating the container walk (`container_gate`).
    /// `None`: no member edit to gate by.
    pub gate: Option<FxHashSet<u32>>,
    /// Containers whose walk stays ungated (`container_gate`).
    pub ungated: FxHashSet<u32>,
    /// Roots a DI framework binds to an interface (`di_bound`): code that runs a change inside one
    /// dispatches to the interface members it overrides, as from a seed's own class.
    pub dispatch_homes: FxHashSet<u32>,
    /// Follow the interface members code that runs a change dispatches to (`Reach::dispatched`);
    /// off with no DI annotations configured.
    pub follow_dispatch: bool,
    /// Dispatch rounds `unnamed_receiver_reach` runs after its first.
    pub dispatch_rounds: usize,
    /// Dispatch rounds follow resolved calls and interface holders only (`typed_round`).
    pub typed_dispatch: bool,
    /// Which DI graphs leave an implementation out (`DiGraphs`).
    pub di_graphs: std::rc::Rc<DiGraphs>,
    /// Per file: can its module's tests run the edited code at all (the edit's modules are on its
    /// test classpath)? Empty: no restriction. Bounds what `unnamed_receiver_reach` reaches, since
    /// a call through an interface is how reach escapes module visibility.
    pub classpath_ok: std::rc::Rc<Vec<bool>>,
}

/// Roots a DI framework binds: annotated with a binding annotation, or referenced from a provider
/// function (`@Provides fun repo(): Repo = RepoImpl()`, `@Binds fun bind(r: RepoImpl): Repo`).
/// DI graph types and the implementations each leaves out (`excludes = [Impl::class]`): a test
/// whose every graph excludes an implementation can never hold it, so a change to it reaches the
/// test through the interface only as a fake or another binding.
#[derive(Default)]
pub struct DiGraphs {
    graphs: FxHashSet<u32>,
    /// graph root -> excluded class roots
    excludes: FxHashMap<u32, FxHashSet<u32>>,
    /// test root -> graph roots it can reach (lazily)
    of_test: std::cell::OnceCell<FxHashMap<u32, FxHashSet<u32>>>,
    /// implementation root -> test roots that name it outside a graph (lazily)
    direct: std::cell::RefCell<FxHashMap<u32, FxHashSet<u32>>>,
    /// implementation root -> roots that can hold it, at any depth (lazily)
    holders: std::cell::RefCell<FxHashMap<u32, FxHashSet<u32>>>,
}

/// Root-level hops a test is searched for from a graph or an implementation.
const OBTAIN_DEPTH: usize = 6;

impl DiGraphs {
    /// `text_of(file)` reads a file's current text.
    pub fn new(
        v: &View,
        graph_annotations: &[String],
        text_of: &dyn Fn(&str) -> Option<String>,
    ) -> DiGraphs {
        let ann: FxHashSet<u32> = graph_annotations
            .iter()
            .filter_map(|a| v.string_id(a))
            .collect();
        let mut out = DiGraphs::default();
        if ann.is_empty() {
            return out;
        }
        let mut texts: FxHashMap<u32, Option<String>> = FxHashMap::default();
        let mut by_name: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
        for c in 0..v.n as u32 {
            let ci = c as usize;
            if v.parent[ci] < 0 && matches!(v.kind[ci], KIND_CLASS | KIND_OBJECT) {
                by_name.entry(v.p.str(v.name[ci])).or_default().push(c);
            }
        }
        for n in 0..v.n as u32 {
            let i = n as usize;
            if v.parent[i] >= 0 || !v.p.node_annotations(i).iter().any(|a| ann.contains(a)) {
                continue;
            }
            out.graphs.insert(n);
            let f = v.file[i];
            let Some(text) = texts
                .entry(f)
                .or_insert_with(|| text_of(v.p.file_path(f as usize)))
                .as_deref()
            else {
                continue;
            };
            let names = excluded_names(text, v.lo[i], v.hi[i]);
            if names.is_empty() {
                continue;
            }
            // A unique class of that name, else the one in the graph's package or imported by name;
            // an ambiguous name stays unresolved (the graph then counts as binding it).
            let pkg = v.p.file_package(f as usize);
            let mut ex = FxHashSet::default();
            for name in &names {
                let cands: Vec<u32> = by_name.get(name.as_str()).cloned().unwrap_or_default();
                let pick: Vec<u32> = if cands.len() == 1 {
                    cands
                } else {
                    cands
                        .into_iter()
                        .filter(|&c| {
                            let cp = v.p.file_package(v.file[c as usize] as usize);
                            cp == pkg || text.contains(&format!("import {cp}.{name}"))
                        })
                        .collect()
                };
                if pick.len() == 1 {
                    ex.insert(pick[0]);
                }
            }
            if !ex.is_empty() {
                out.excludes.insert(n, ex);
            }
        }
        out
    }

    /// Roots reachable backwards from `from` over references, root to root, skipping `skip`.
    fn referrers(v: &View, from: u32, skip: &FxHashSet<u32>) -> FxHashSet<u32> {
        let mut seen: FxHashSet<u32> = [from].into_iter().collect();
        let mut frontier = vec![from];
        for _ in 0..OBTAIN_DEPTH {
            let mut next = Vec::new();
            for r in frontier {
                for m in r..v.n as u32 {
                    if m != r && v.root[m as usize] != r {
                        break;
                    }
                    for &x in v.refby(m) {
                        let xr = v.root[x as usize];
                        if !skip.contains(&xr) && seen.insert(xr) {
                            next.push(xr);
                        }
                    }
                }
            }
            frontier = next;
        }
        seen
    }

    /// Does any graph leave `i` out?
    pub fn excludable(&self, i: u32) -> bool {
        self.excludes.values().any(|e| e.contains(&i))
    }

    /// Can test `t` never hold any of `impls`? Only when every graph it reaches excludes each of
    /// them and it names none outside a graph; reaching no graph at all is "can't tell".
    pub fn blocked(&self, v: &View, t: u32, impls: &FxHashSet<u32>) -> bool {
        if self.excludes.is_empty() || impls.is_empty() {
            return false;
        }
        let of_test = self.of_test.get_or_init(|| {
            let mut m: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
            let none = FxHashSet::default();
            for &g in &self.graphs {
                for r in DiGraphs::referrers(v, g, &none) {
                    if v.test_class.contains_key(&r) {
                        m.entry(r).or_default().insert(g);
                    }
                }
            }
            m
        });
        let Some(graphs) = of_test.get(&t) else {
            return false;
        };
        impls.iter().all(|&i| {
            if !graphs.iter().all(|g| {
                self.excludes
                    .get(g)
                    .map(|e| e.contains(&i))
                    .unwrap_or(false)
            }) {
                return false;
            }
            let mut direct = self.direct.borrow_mut();
            let named = direct
                .entry(i)
                .or_insert_with(|| DiGraphs::referrers(v, i, &self.graphs));
            !named.contains(&t)
        })
    }

    /// Can test `t` never run edited implementation `i`? Only when every graph it reaches excludes
    /// `i` and nothing it reaches, at any depth, names `i` or a graph that binds it. The depth
    /// matters here: the test reaches `i` through its interface either way, so only its graphs
    /// and the code that names `i` decide.
    pub fn never_holds(&self, v: &View, t: u32, i: u32) -> bool {
        if !self.excludable(i) || !self.blocked(v, t, &[i].into_iter().collect()) {
            return false;
        }
        let mut holders = self.holders.borrow_mut();
        let can = holders.entry(i).or_insert_with(|| {
            let excluding: FxHashSet<u32> = self
                .excludes
                .iter()
                .filter(|(_, e)| e.contains(&i))
                .map(|(&g, _)| g)
                .collect();
            let mut seen: FxHashSet<u32> = [i].into_iter().collect();
            seen.extend(self.graphs.iter().filter(|g| !excluding.contains(g)));
            let mut stack: Vec<u32> = seen.iter().copied().collect();
            while let Some(r) = stack.pop() {
                for m in r..v.n as u32 {
                    if m != r && v.root[m as usize] != r {
                        break;
                    }
                    for &x in v.refby(m) {
                        let xr = v.root[x as usize];
                        if !excluding.contains(&xr) && seen.insert(xr) {
                            stack.push(xr);
                        }
                    }
                }
            }
            seen
        });
        !can.contains(&t)
    }
}

/// `X` of each `X::class` in a `@...Graph(excludes = [...])` argument within lines lo..=hi, through
/// import aliases.
fn excluded_names(text: &str, lo: u32, hi: u32) -> FxHashSet<String> {
    let lines: Vec<&str> = text.lines().collect();
    let a = (lo as usize).saturating_sub(1).min(lines.len());
    let b = (hi as usize).min(lines.len());
    let span = lines[a..b].join("\n");
    let mut out = FxHashSet::default();
    let Some(at) = span.find("excludes") else {
        return out;
    };
    let rest = &span[at..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else {
        return out;
    };
    if close < open {
        return out;
    }
    let aliases: FxHashMap<&str, &str> = text
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix("import ")?;
            let (path, alias) = l.split_once(" as ")?;
            Some((alias.trim(), path.rsplit('.').next()?.trim()))
        })
        .collect();
    for part in rest[open + 1..close].split(',') {
        let name = part
            .trim()
            .strip_suffix("::class")
            .map(|n| n.trim().rsplit('.').next().unwrap_or(n).trim());
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            out.insert(aliases.get(n).copied().unwrap_or(n).to_string());
        }
    }
    out
}

pub fn di_bound(v: &View, binding: &[String], providers: &[String]) -> FxHashSet<u32> {
    let bind: FxHashSet<u32> = binding.iter().filter_map(|a| v.string_id(a)).collect();
    let prov: FxHashSet<u32> = providers.iter().filter_map(|a| v.string_id(a)).collect();
    let mut provider_fns: FxHashSet<u32> = FxHashSet::default();
    let mut out: FxHashSet<u32> = FxHashSet::default();
    for n in 0..v.n as u32 {
        let anns = v.p.node_annotations(n as usize);
        if anns.is_empty() {
            continue;
        }
        let i = n as usize;
        if matches!(v.kind[i], KIND_CLASS | KIND_OBJECT) && anns.iter().any(|a| bind.contains(a)) {
            out.insert(v.root[i]);
        } else if anns.iter().any(|a| prov.contains(a)) {
            provider_fns.insert(n);
        }
    }
    if !provider_fns.is_empty() {
        for n in 0..v.n as u32 {
            if v.kind[n as usize] == KIND_CLASS
                && v.refby(n).iter().any(|r| provider_fns.contains(r))
            {
                out.insert(v.root[n as usize]);
            }
        }
    }
    out
}

struct Overrides<'v, 'a> {
    v: &'v View<'a>,
    done: FxHashSet<u32>,
    by_container: FxHashMap<i32, (FxHashMap<u32, Vec<u32>>, FxHashSet<u32>)>,
    types: FxHashSet<u32>,
}

impl<'v, 'a> Overrides<'v, 'a> {
    fn new(v: &'v View<'a>) -> Self {
        Overrides {
            v,
            done: Default::default(),
            by_container: Default::default(),
            types: Default::default(),
        }
    }

    fn of(&mut self, n: u32) -> Vec<u32> {
        let v = self.v;
        if self.done.contains(&n) || v.flags[n as usize] & (N_PRIVATE | N_NO_WALK) != 0 {
            return vec![];
        }
        self.done.insert(n);
        let c = v.parent[n as usize];
        if !self.by_container.contains_key(&c) {
            let mut names: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
            let seen = if c >= 0 {
                v.supertypes(c as u32)
            } else {
                FxHashSet::default()
            };
            for &t in &seen {
                if t as i32 != c
                    && v.parent[t as usize] >= 0
                    && v.flags[t as usize] & N_PRIVATE == 0
                {
                    names.entry(v.name[t as usize]).or_default().push(t);
                }
            }
            self.by_container.insert(c, (names, seen));
        }
        let got = &self.by_container[&c];
        let hits = got.0.get(&v.name[n as usize]).cloned().unwrap_or_default();
        if !hits.is_empty() {
            self.types.extend(got.1.iter().copied());
        }
        hits
    }
}

/// The name gate for the container walk, and the containers it leaves alone.
///
/// A member edit walks from the member's containers to every member of their supertypes, so a body
/// change to `RepoImpl.update` reaches every caller of `Repo.insert`. Only
/// two things there can observe the edit: a supertype member whose own code calls a changed member
/// (a template method calling `hook()` on `this`), and a holder of the type that calls one. Both
/// write the name of a member that runs the edit — a changed member, or one in the edited class that
/// calls it (`runs_names`, grown by the caller to a fixpoint) — so the gate is the set of declarations
/// that do. A holder that only passes the value on is followed to its referrers, tentatively.
///
/// A container's walk stays full when a seed inside it is construction-time code, an implicitly
/// called member (operators, `toString`, `hashCode`), which no call site names, or an interface
/// member (a contract change every implementer sees), or when the
/// container sits in a DI graph (its wiring changed). A graph is a type carrying one of
/// `graph_annotations` or named in `graph_types`. A whole-type seed walks from itself, ungated.
pub fn container_gate(
    v: &View,
    graph_annotations: &[String],
    graph_types: &[String],
    seeds: &FxHashSet<u32>,
    runs_names: &FxHashSet<u32>,
) -> (Option<FxHashSet<u32>>, FxHashSet<u32>) {
    let ann: FxHashSet<u32> = graph_annotations
        .iter()
        .filter_map(|a| v.string_id(a))
        .collect();
    let named: FxHashSet<u32> = graph_types.iter().filter_map(|t| v.string_id(t)).collect();
    let is_graph = |r: u32| {
        named.contains(&v.name[r as usize])
            || v.p
                .node_annotations(r as usize)
                .iter()
                .any(|a| ann.contains(a))
    };
    let mut names: FxHashSet<u32> = FxHashSet::default();
    let mut ungated: FxHashSet<u32> = FxHashSet::default();
    for &n in seeds {
        let i = n as usize;
        if v.parent[i] < 0 {
            continue;
        }
        let whole_type = matches!(v.kind[i], KIND_CLASS | KIND_INTERFACE | KIND_OBJECT);
        // An interface member's edit is a contract change every implementer, test fakes included, sees.
        let contract = v.kind[v.parent[i] as usize] == KIND_INTERFACE;
        if whole_type
            || contract
            || v.rides[i]
            || v.implicit.contains(&v.name[i])
            || is_graph(v.root[i])
        {
            let mut p = v.parent[i];
            while p >= 0 {
                ungated.insert(p as u32);
                p = v.parent[p as usize];
            }
            continue;
        }
        names.insert(v.name[i]);
    }
    if names.is_empty() {
        return (None, ungated);
    }
    names.extend(runs_names.iter().copied());
    let mut gate: FxHashSet<u32> = FxHashSet::default();
    for &w in &names {
        gate.extend(v.fam(Family::NameUsers, w).iter().copied());
    }
    for n in 0..v.n as u32 {
        if v.parent[n as usize] >= 0 && names.contains(&v.name[n as usize]) {
            gate.extend(v.refby(n).iter().copied());
        }
    }
    (Some(gate), ungated)
}

/// A declaration that can hand a value on: a property or function, not a type.
fn is_holder(v: &View, n: u32) -> bool {
    !matches!(
        v.kind[n as usize],
        KIND_CLASS | KIND_INTERFACE | KIND_OBJECT
    )
}

/// symbol_closure's answer: every reached node, which only a walk's first hop reached (`partial`),
/// what runs changed code (`altered`) and what runs a seed (`runs`).
#[derive(Default)]
pub struct Reach {
    /// First reacher of each node (`KT_AFFECTED_TRACE` set only), with the step that reached it.
    pub via: FxHashMap<u32, (u32, &'static str)>,
    pub state: FxHashMap<u32, u8>,
    pub altered: FxHashSet<u32>,
    pub runs: FxHashSet<u32>,
    /// Interface members a DI-bound implementation that runs the change dispatches to
    /// (`Routes::dispatch_homes`): their callers hold the interface, often through an accessor
    /// chain the linker cannot type, so `unnamed_receiver_reach` follows them like a seed's own.
    pub dispatched: FxHashSet<u32>,
    /// Each dispatched member's sources: the roots whose code dispatched to it.
    pub dispatched_by: FxHashMap<u32, FxHashSet<u32>>,
}

impl Reach {
    pub fn contains(&self, n: u32) -> bool {
        self.state.contains_key(&n)
    }
    pub fn partial(&self, n: u32) -> bool {
        self.state.get(&n) == Some(&1)
    }
    pub fn nodes(&self) -> impl Iterator<Item = u32> + '_ {
        self.state.keys().copied()
    }
}

pub fn symbol_closure(
    v: &View,
    routes: &Routes,
    seeds: &FxHashSet<u32>,
    from_containers: bool,
    edited: bool,
) -> Reach {
    let (parent, flags, root) = (&v.parent, &v.flags, &v.root);
    let mut homes: FxHashSet<u32> = seeds.iter().map(|&n| root[n as usize]).collect();
    for &n in seeds {
        homes.extend(v.fam(Family::ExtRecv, n).iter().map(|&t| root[t as usize]));
    }
    // Every provided class is a home: constructing it runs code the change may alter.
    for n in 0..v.n as u32 {
        if v.in_builds(n) {
            homes.insert(root[n as usize]);
        }
    }
    let mut ancestors: FxHashSet<u32> = FxHashSet::default();
    if from_containers {
        for &n in seeds {
            let mut p = parent[n as usize];
            while p >= 0 && !ancestors.contains(&(p as u32)) {
                ancestors.insert(p as u32);
                p = parent[p as usize];
            }
        }
    }
    // Holders followed from a gated type (see `container_gate`); members of the set are not reached.
    let mut tentative: FxHashSet<u32> = FxHashSet::default();
    let tracing = TRACE.load(Ordering::Relaxed) || std::env::var_os("KT_AFFECTED_TRACE").is_some();
    let mut via: FxHashMap<u32, (u32, &'static str)> = FxHashMap::default();
    let origins: FxHashSet<u32> = seeds.union(&ancestors).copied().collect();
    // A walk that starts at a routed handler never enters the dispatch bases, through any number of
    // intermediate generic bases; one from any other origin walks everything. Two walks, so the
    // answer does not depend on visiting order.
    let walk_from = |start: &mut dyn Iterator<Item = u32>, block_bases: bool| -> FxHashSet<u32> {
        let mut seen: FxHashSet<u32> = FxHashSet::default();
        let mut queue: Vec<u32> = Vec::new();
        for n in start {
            if seen.insert(n) {
                queue.push(n);
            }
        }
        while let Some(n) = queue.pop() {
            if flags[n as usize] & N_NO_WALK != 0 {
                continue;
            }
            let mut nxt: Vec<u32> = v.declares(n).to_vec();
            if block_bases {
                nxt.retain(|t| !routes.bases.contains(&root[*t as usize]));
            }
            if flags[n as usize] & N_IN_TEST == 0 {
                nxt.extend_from_slice(v.fam(Family::Anon, n));
            }
            for t in nxt {
                if seen.insert(t) {
                    queue.push(t);
                }
            }
        }
        seen
    };
    let routed = |n: &u32| routes.routes.contains_key(&root[*n as usize]);
    let mut walked = walk_from(&mut origins.iter().copied().filter(|n| routed(n)), true);
    let unrouted = walk_from(&mut origins.iter().copied().filter(|n| !routed(n)), false);
    // Members only a routed handler's supertype walk reaches that implement a dispatch base's
    // member (a template method's `handle`): code calling them dispatches through the base, by
    // event, which the route already answers. Reached virtually only, they hand nothing on.
    let dispatch_entry: FxHashSet<u32> = walked
        .iter()
        .copied()
        .filter(|&n| {
            let p = parent[n as usize];
            p >= 0
                && !unrouted.contains(&n)
                && !routes.routes.contains_key(&root[n as usize])
                && v.supertypes(p as u32).iter().any(|&t| {
                    let tp = parent[t as usize];
                    tp >= 0
                        && tp != p
                        && v.name[t as usize] == v.name[n as usize]
                        && routes.bases.contains(&root[t as usize])
                })
        })
        .collect();
    walked.extend(unrouted);
    let mut virtual_: FxHashSet<u32> = walked.difference(&origins).copied().collect();
    // What only the walk from a seed's containers added: gated by `routes.gate` (see container_gate).
    let mut container_only: FxHashSet<u32> = FxHashSet::default();
    if routes.gate.is_some() && !ancestors.is_empty() {
        // Walks that stay full: from the seeds themselves and from containers the gate leaves alone.
        let mut from_seeds: FxHashSet<u32> = seeds.clone();
        from_seeds.extend(
            ancestors
                .iter()
                .copied()
                .filter(|a| routes.ungated.contains(a)),
        );
        let mut stack: Vec<u32> = from_seeds.iter().copied().collect();
        while let Some(n) = stack.pop() {
            if flags[n as usize] & N_NO_WALK != 0 {
                continue;
            }
            let mut nxt: Vec<u32> = v.declares(n).to_vec();
            if flags[n as usize] & N_IN_TEST == 0 {
                nxt.extend_from_slice(v.fam(Family::Anon, n));
            }
            for t in nxt {
                if from_seeds.insert(t) {
                    stack.push(t);
                }
            }
        }
        // Members of an ungated container stay full however the walk reached them (an interface
        // member's siblings arrive through each implementer's walk).
        container_only = virtual_
            .difference(&from_seeds)
            .copied()
            .filter(|&n| {
                v.parent[n as usize] < 0 || !routes.ungated.contains(&(v.parent[n as usize] as u32))
            })
            .collect();
    }
    // A supertype member that calls a changed member on `this` (a template method) hands the change
    // to whoever holds the type, so holders are gated only when no such member exists.
    let gate_holders = match &routes.gate {
        Some(g) => !container_only
            .iter()
            .any(|&n| v.parent[n as usize] >= 0 && g.contains(&n)),
        None => false,
    };
    let mut any_members: FxHashSet<u32> = FxHashSet::default();
    for &n in &origins {
        let chain = v.supertypes(n);
        if chain
            .iter()
            .any(|&t| flags[t as usize] & N_ANY_MEMBERS != 0)
        {
            any_members.extend(chain);
        }
    }
    let mut state: FxHashMap<u32, u8> = seeds.iter().map(|&n| (n, 2u8)).collect();
    for &n in &virtual_ {
        state.entry(n).or_insert(1);
    }
    for r in homes.iter() {
        if let Some(es) = routes.routes.get(r) {
            for &e in es {
                if !state.contains_key(&e) {
                    if tracing {
                        via.entry(e).or_insert((*r, "route"));
                    }
                    state.insert(e, 1);
                    virtual_.insert(e);
                }
            }
        }
    }
    // Routed event -> the handler roots routed by it.
    let mut handlers_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    for (&h, es) in &routes.routes {
        for &e in es {
            handlers_of.entry(e).or_default().insert(h);
        }
    }
    let mut dispatched: FxHashSet<u32> = FxHashSet::default();
    let mut dispatched_by: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    let mut altered: FxHashSet<u32> = seeds.clone();
    let mut reshaped: FxHashSet<u32> = seeds.clone();
    let mut runs: FxHashSet<u32> = seeds.clone();
    // Order does not change the fixpoint.
    let mut queue: Vec<u32> = state.keys().copied().collect();
    queue.sort_unstable();
    let mut di_overrides = Overrides::new(v);
    let mut overrides = Overrides::new(v);
    let mut ups: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    let mut provided: FxHashSet<(u32, u32)> = FxHashSet::default();
    let mut built: FxHashSet<u32> = seeds.clone();
    let mut revisit: FxHashSet<u32> = FxHashSet::default();
    let mut sizes = (walked.len(), overrides.types.len(), any_members.len());
    let is_type = |n: u32| {
        matches!(
            v.kind[n as usize],
            KIND_CLASS | KIND_INTERFACE | KIND_OBJECT
        )
    };

    loop {
        let n = match queue.pop() {
            Some(n) => n,
            None => {
                if revisit.is_empty() {
                    break;
                }
                let now = (walked.len(), overrides.types.len(), any_members.len());
                if now == sizes {
                    break;
                }
                sizes = now;
                let mut r: Vec<u32> = revisit.drain().collect();
                r.sort_unstable();
                queue.extend(r);
                continue;
            }
        };
        let sn = state[&n];
        let ni = n as usize;
        let di_home = !homes.contains(&root[ni]) && routes.dispatch_homes.contains(&root[ni]);
        // Interface members code running the change dispatches to, for `unnamed_receiver_reach`'s
        // dispatch rounds. Recorded only: this walk stays what it is without DI, so dispatch only
        // ever adds reach. An edit's own overrides are those rounds' starting set already; an inner
        // walk's seeds are callers that run it, and a bound one is called through its interface.
        if routes.follow_dispatch
            && sn == 2
            && runs.contains(&n)
            && parent[ni] >= 0
            && (di_home
                || (homes.contains(&root[ni])
                    && (!seeds.contains(&n)
                        || (!edited && routes.dispatch_homes.contains(&root[ni])))))
        {
            for t in di_overrides.of(n) {
                if routes.routes.contains_key(&root[ni]) && routes.bases.contains(&root[t as usize])
                {
                    continue;
                }
                dispatched_by.entry(t).or_default().insert(root[ni]);
                if dispatched.insert(t) && tracing {
                    via.entry(t)
                        .or_insert((n, "DI dispatch: called through its interface"));
                }
            }
        }
        if sn == 2 && runs.contains(&n) && homes.contains(&root[ni]) && parent[ni] >= 0 {
            for t in overrides.of(n) {
                if routes.routes.contains_key(&root[ni]) && routes.bases.contains(&root[t as usize])
                {
                    continue;
                }
                if container_only.remove(&t) {
                    queue.push(t); // code that runs the change dispatches here: expand it ungated
                }
                if !state.contains_key(&t) {
                    if tracing {
                        via.entry(t).or_insert((n, "override"));
                    }
                    state.insert(t, 1);
                    virtual_.insert(t);
                    queue.push(t);
                }
            }
        }
        let mut skip: FxHashSet<u32> = FxHashSet::default();
        if sn == 1 {
            skip.extend(v.fam(Family::VBlock, n).iter().copied());
            let mut held = Vec::new();
            for &r in v.fam(Family::TargOnly, n) {
                // holds(r, n): r's file resolved through a walked type argument that n's container is.
                let p = parent[ni];
                let ok = if p < 0 {
                    true
                } else {
                    let mut ok = false;
                    for &t in v.fam(Family::TargVia, r) {
                        if !walked.contains(&t) && !overrides.types.contains(&t) {
                            continue;
                        }
                        let got = ups.entry(t).or_insert_with(|| v.supertypes(t));
                        if got.contains(&(p as u32)) {
                            ok = true;
                            break;
                        }
                    }
                    ok
                };
                if !ok {
                    held.push(r);
                }
            }
            if !held.is_empty() {
                skip.extend(held);
                revisit.insert(n);
            }
        }
        let asked = flags[ni] & N_ASKED != 0;
        if built.contains(&n) || (!v.in_builds(n) && !asked) {
            for (m, keys) in v.provides(n) {
                if skip.contains(&m) || provided.contains(&(n, m)) {
                    continue;
                }
                provided.insert((n, m));
                let mut chain: FxHashSet<u32> = FxHashSet::default();
                for &k in keys {
                    chain.extend(v.supertypes(k));
                }
                let mine = v.supertypes(n);
                if mine
                    .iter()
                    .chain(chain.iter())
                    .any(|&t| flags[t as usize] & N_ANY_MEMBERS != 0)
                {
                    any_members.extend(chain.iter().copied());
                }
                for &t in &chain {
                    walked.insert(t);
                    let fresh = built.insert(t);
                    if !state.contains_key(&t) {
                        if tracing {
                            via.entry(t).or_insert((n, "provides"));
                        }
                        state.insert(t, 1);
                        virtual_.insert(t);
                        queue.push(t);
                    } else if fresh && !v.fam(Family::Provides, t).is_empty() {
                        queue.push(t);
                    }
                }
            }
        }
        if sn == 1 && flags[ni] & N_MARKER != 0 && !any_members.contains(&n) {
            revisit.insert(n);
            continue;
        }
        let mut refs: Vec<u32> = v.refby(n).to_vec();
        // An unchanged event a routed handler reaches is referenced by that handler's own header
        // (`: Handler<Event>`): that edge carries nothing back into the handler, whose holders the
        // seed's own walk decides.
        if sn == 1
            && let Some(hs) = handlers_of.get(&n)
        {
            refs.retain(|r| !hs.contains(r));
        }
        if sn == 1 && dispatch_entry.contains(&n) {
            refs.clear();
        }
        let routed = v.fam(Family::KoinRouted, n);
        if !routed.is_empty() && !(edited && seeds.contains(&n)) {
            refs.retain(|r| !routed.contains(r));
        }
        if sn == 1
            && container_only.contains(&n)
            && let Some(gate) = &routes.gate
        {
            if is_type(n) {
                if gate_holders {
                    // A holder passes the value on: follow its referrers tentatively and keep the
                    // first along each chain that writes a gated name; the rest never see the edit.
                    let mut keep: Vec<u32> = Vec::new();
                    let mut stack: Vec<u32> = Vec::new();
                    for &r in &refs {
                        if gate.contains(&r) {
                            keep.push(r);
                        } else if tentative.insert(r) {
                            stack.push(r);
                        }
                    }
                    while let Some(t) = stack.pop() {
                        for &r2 in v.refby(t) {
                            if state.contains_key(&r2) {
                                continue;
                            }
                            if gate.contains(&r2) {
                                keep.push(r2);
                            } else if tentative.insert(r2) {
                                stack.push(r2);
                            }
                        }
                    }
                    refs = keep;
                }
            } else if !gate.contains(&n) {
                refs.clear();
            }
        }
        let mut weak: FxHashSet<u32> = FxHashSet::default();
        if sn == 1 {
            refs.retain(|r| !skip.contains(r));
        } else if altered.contains(&n) {
            weak.extend(v.fam(Family::VBlock, n).iter().copied());
        }
        let spread = sn == 1 || altered.contains(&n);
        for r in refs {
            let ri = r as usize;
            let want = if sn == 1 && virtual_.contains(&r) {
                1
            } else {
                2
            };
            let mut push = false;
            if v.in_builds(r)
                && !built.contains(&r)
                && v.builds(r).map(|b| b.contains(&n)).unwrap_or(true)
            {
                built.insert(r);
                push = true;
            }
            if state.get(&r).copied().unwrap_or(0) < want {
                if tracing {
                    via.entry(r)
                        .or_insert((n, if sn == 1 { "ref-of-virtual" } else { "ref" }));
                }
                state.insert(r, want);
                push = true;
            }
            if spread && parent[ri] == n as i32 {
                if reshaped.contains(&n) && !reshaped.contains(&r) {
                    altered.insert(r);
                    reshaped.insert(r);
                    push = true;
                }
            } else if spread {
                if altered.insert(r) {
                    push = true;
                }
                if !runs.contains(&r) && (sn == 1 || runs.contains(&n)) {
                    runs.insert(r);
                    push = true;
                }
                if !reshaped.contains(&r) && !weak.contains(&r) {
                    reshaped.insert(r);
                    push = true;
                }
            }
            if push {
                queue.push(r);
            }
        }
    }
    if capturing() {
        for (&n, &(from, how)) in &via {
            note(n, from, || how.to_string());
        }
    }
    Reach {
        via,
        state,
        altered,
        runs,
        dispatched,
        dispatched_by,
    }
}

/// Callers of a changed member whose receiver's type is never written.
pub fn unnamed_receiver_reach(
    v: &View,
    routes: &Routes,
    seeds: &FxHashSet<u32>,
    reached: &Reach,
) -> FxHashSet<u32> {
    // The inner closures seed whole containers: the name gate belongs to the edit's own closure.
    let ungated_routes = Routes {
        routes: routes.routes.clone(),
        bases: routes.bases.clone(),
        gate: None,
        ungated: FxHashSet::default(),
        dispatch_homes: routes.dispatch_homes.clone(),
        follow_dispatch: routes.follow_dispatch,
        dispatch_rounds: routes.dispatch_rounds,
        typed_dispatch: routes.typed_dispatch,
        di_graphs: routes.di_graphs.clone(),
        classpath_ok: routes.classpath_ok.clone(),
    };
    let routes = &ungated_routes;
    let (parent, flags, root) = (&v.parent, &v.flags, &v.root);
    let mut seed_roots: FxHashSet<u32> = seeds
        .iter()
        .filter(|&&n| parent[n as usize] >= 0)
        .map(|&n| root[n as usize])
        .collect();
    for &n in seeds {
        seed_roots.extend(v.fam(Family::ExtRecv, n).iter().map(|&t| root[t as usize]));
    }
    let mut changed: FxHashSet<u32> = seeds
        .iter()
        .copied()
        .chain(reached.nodes())
        .filter(|&n| {
            let i = n as usize;
            !reached.partial(n)
                && reached.altered.contains(&n)
                && seed_roots.contains(&root[i])
                && parent[i] >= 0
                && flags[i] & N_PRIVATE == 0
                && !v.rides[i]
        })
        .collect();
    let mut overrides = Overrides::new(v);
    for n in changed.clone() {
        for t in overrides.of(n) {
            if seed_roots.contains(&root[t as usize]) && !v.rides[t as usize] {
                changed.insert(t);
            }
        }
    }
    // An interface member that code running the change dispatches to is followed like a seed's own,
    // round by round: each round's closure can dispatch to further interface members (an impl
    // reached only through an accessor chain calls another bound service). Bounded; the rounds
    // only ever add reach.
    let eligible = |t: u32| {
        let i = t as usize;
        parent[i] >= 0 && flags[i] & N_PRIVATE == 0 && !v.rides[i]
    };
    // Round 0 is the walk without DI; the dispatch rounds only add to it. They see no main reach:
    // what the main walk reached is not expanded through receivers there.
    let (mut out, first_by) = receiver_round(
        v,
        routes,
        &changed,
        &seed_roots,
        reached,
        &FxHashSet::default(),
    );
    let off_classpath = |n: u32| {
        let r = root[n as usize];
        v.test_class.contains_key(&r)
            && !routes
                .classpath_ok
                .get(v.file[r as usize] as usize)
                .copied()
                .unwrap_or(true)
    };
    out.retain(|&n| !off_classpath(n));
    let mut handled: FxHashSet<u32> = changed.clone();
    let mut pending: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    merge_by(&mut pending, &reached.dispatched_by);
    merge_by(&mut pending, &first_by);
    pending.retain(|&t, _| eligible(t) && !handled.contains(&t));
    let none = Reach::default();
    for _ in 0..routes.dispatch_rounds {
        if pending.is_empty() {
            break;
        }
        seed_roots.extend(pending.keys().map(|&t| root[t as usize]));
        handled.extend(pending.keys().copied());
        // A test the round reaches observes the change only if its object graph can hold an
        // implementation that dispatched (`DiGraphs::blocked`): targets whose dispatchers some graph
        // leaves out walk per dispatcher set, the rest together. Every group sees the same prior
        // reach, so one never hides another's.
        let mut groups: BTreeMap<Vec<u32>, FxHashSet<u32>> = BTreeMap::new();
        for (&t, by) in &pending {
            let key: Vec<u32> = if by.iter().any(|&i| !routes.di_graphs.excludable(i)) {
                Vec::new()
            } else {
                let mut k: Vec<u32> = by.iter().copied().collect();
                k.sort_unstable();
                k
            };
            groups.entry(key).or_default().insert(t);
        }
        let prior = out.clone();
        let mut next: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
        for (impls, targets) in groups {
            let impls: FxHashSet<u32> = impls.into_iter().collect();
            let (got, by) = if routes.typed_dispatch {
                typed_round(v, routes, &targets, &seed_roots, &prior)
            } else {
                receiver_round(v, routes, &targets, &seed_roots, &none, &prior)
            };
            for n in got {
                let r = root[n as usize];
                if !v.test_class.contains_key(&r)
                    || (!off_classpath(n) && !routes.di_graphs.blocked(v, r, &impls))
                {
                    out.insert(n);
                }
            }
            merge_by(&mut next, &by);
        }
        next.retain(|&t, _| eligible(t) && !handled.contains(&t));
        pending = next;
    }
    if !pending.is_empty() {
        eprintln!(
            "kt-affected: DI dispatch stopped at dispatch_rounds = {} with {} interface members still to walk; \
             the selection may miss tests behind deeper chains (raise [di] dispatch_rounds)",
            routes.dispatch_rounds,
            pending.len()
        );
    }
    out
}

/// One round of `unnamed_receiver_reach` over `changed`: the nodes it reaches, and the interface
/// members its closures dispatch to.
fn receiver_round(
    v: &View,
    routes: &Routes,
    changed: &FxHashSet<u32>,
    seed_roots: &FxHashSet<u32>,
    reached: &Reach,
    prior: &FxHashSet<u32>,
) -> (FxHashSet<u32>, FxHashMap<u32, FxHashSet<u32>>) {
    let (parent, flags, name, root) = (&v.parent, &v.flags, &v.name, &v.root);
    let mut dispatched: FxHashSet<u32> = FxHashSet::default();
    let mut dispatched_by: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    let mut gated: FxHashMap<u32, std::collections::BTreeSet<u32>> = FxHashMap::default();
    let mut ungated: FxHashSet<u32> = FxHashSet::default();
    for &n in changed {
        let implicit = v.implicit.contains(&name[n as usize]);
        let mut p = parent[n as usize];
        while p >= 0 {
            note(p as u32, n, || "container of the changed member".into());
            if implicit {
                ungated.insert(p as u32);
            } else {
                gated.entry(p as u32).or_default().insert(name[n as usize]);
            }
            p = parent[p as usize];
        }
    }
    let mut out: FxHashSet<u32> = FxHashSet::default();
    let fresh: FxHashSet<u32> = ungated
        .iter()
        .copied()
        .filter(|&n| !reached.contains(n) && !prior.contains(&n))
        .collect();
    if !fresh.is_empty() {
        let r = symbol_closure(v, routes, &fresh, true, false);
        dispatched.extend(r.dispatched.iter().copied());
        merge_by(&mut dispatched_by, &r.dispatched_by);
        out.extend(r.nodes());
    }
    let mut by_names: BTreeMap<Vec<u32>, FxHashSet<u32>> = BTreeMap::new();
    for (&c, words) in &gated {
        if (!reached.contains(c) || reached.partial(c)) && !out.contains(&c) && !prior.contains(&c)
        {
            by_names
                .entry(words.iter().copied().collect())
                .or_default()
                .insert(c);
        }
    }
    let mut keep: FxHashSet<u32> = FxHashSet::default();
    for (words, containers) in by_names {
        let mut users: FxHashSet<u32> = FxHashSet::default();
        for &w in &words {
            users.extend(v.fam(Family::NameUsers, w).iter().copied());
        }
        if users.is_empty() {
            continue;
        }
        let test_roots: FxHashSet<u32> = users
            .iter()
            .map(|&n| root[n as usize])
            .filter(|r| v.test_class.contains_key(r))
            .collect();
        let observes = |n: u32| {
            (users.contains(&n) || test_roots.contains(&root[n as usize]))
                && !seed_roots.contains(&root[n as usize])
        };
        // A holder of the container (`TestGraph.a`) is read through receivers the linker cannot type
        // (`graph.a`, `graph` inferred), so follow whatever writes its name, through further holders.
        // A member carrying a changed name is the edit's own counterpart: its callers are the users above.
        let mut holders: Vec<u32> = Vec::new();
        let mut seen: FxHashSet<u32> = FxHashSet::default();
        let mut seers_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
        // The containers' own supertypes declare members, not holders of the type.
        let own: FxHashSet<u32> = containers
            .iter()
            .flat_map(|&c| v.supertypes(root[c as usize]))
            .map(|t| root[t as usize])
            .collect();
        for n in symbol_closure(v, routes, &containers, true, false).nodes() {
            if out.contains(&n) || prior.contains(&n) || reached.runs.contains(&n) {
                continue;
            }
            if observes(n) {
                let names: Vec<&str> = words.iter().map(|&x| v.p.str(x)).collect();
                if let Ok(mut g) = EXPLAIN_VIA.lock()
                    && let Some(m) = g.as_mut()
                    && let Some(e) = m.get_mut(&n)
                    && !e.1.contains("unnamed receiver")
                {
                    e.1 = format!("{}; unnamed receiver: calls `{}`", e.1, names.join("`/`"));
                }
                keep.insert(n);
            }
        }
        // A holder references a container directly (`val a: ServiceA = ServiceA()`), or a supertype
        // declaring a changed name, which can hand the container out (`val a: Service`, a DI
        // graph's own accessor): `a.m()` on it can dispatch to the change.
        let mut held: Vec<u32> = containers.iter().copied().collect();
        for &c in &containers {
            for t in v.supertypes(root[c as usize]) {
                let p = v.parent[t as usize];
                if p >= 0
                    && root[p as usize] != root[c as usize]
                    && flags[t as usize] & N_PRIVATE == 0
                    && words.contains(&name[t as usize])
                {
                    held.push(p as u32);
                }
            }
        }
        held.sort_unstable();
        held.dedup();
        for &c in &held {
            for &n in v.refby(c) {
                if is_holder(v, n)
                    && !words.contains(&name[n as usize])
                    && !seed_roots.contains(&root[n as usize])
                    && !own.contains(&root[n as usize])
                    && seen.insert(n)
                {
                    note(n, c, || "holds the changed type".into());
                    holders.push(n);
                }
            }
        }
        while let Some(h) = holders.pop() {
            // A private holder is read only inside its own declaration.
            let private = flags[h as usize] & N_PRIVATE != 0;
            // Whoever reads `x.h` holds the holder's container somewhere in its own class.
            let holder_root = root[h as usize];
            let seers = seers_of.entry(holder_root).or_insert_with(|| {
                // Through up to three wrappers (`ext.graph.a`).
                let mut seen: FxHashSet<u32> = FxHashSet::default();
                let mut frontier = vec![holder_root];
                for _ in 0..3 {
                    let mut next = Vec::new();
                    for r in frontier {
                        for &x in v.refby(r) {
                            let xr = root[x as usize];
                            if seen.insert(xr) {
                                next.push(xr);
                            }
                        }
                    }
                    frontier = next;
                }
                seen
            });
            for &w in v.fam(Family::NameUsers, name[h as usize]) {
                if private && root[w as usize] != holder_root {
                    continue;
                }
                if root[w as usize] != holder_root && !seers.contains(&root[w as usize]) {
                    continue;
                }
                if !seen.insert(w)
                    || out.contains(&w)
                    || prior.contains(&w)
                    || reached.runs.contains(&w)
                {
                    continue;
                }
                // Only a declaration that names both the holder and a changed member reads it here.
                if users.contains(&w) && !seed_roots.contains(&root[w as usize]) {
                    note(w, h, || {
                        let names: Vec<&str> = words.iter().map(|&x| v.p.str(x)).collect();
                        format!(
                            "unnamed receiver: reads `{}` and calls `{}`",
                            v.p.str(name[h as usize]),
                            names.join("`/`")
                        )
                    });
                    keep.insert(w);
                } else if is_holder(v, w) && !words.contains(&name[w as usize]) {
                    note(w, h, || format!("holds `{}`", v.p.str(name[h as usize])));
                    holders.push(w);
                }
            }
        }
    }
    if !keep.is_empty() {
        let r = symbol_closure(v, routes, &keep, false, false);
        dispatched.extend(r.dispatched.iter().copied());
        merge_by(&mut dispatched_by, &r.dispatched_by);
        out.extend(r.nodes());
    }
    let _ = dispatched;
    (out, dispatched_by)
}

/// A dispatch round over interface members `targets`, by type: code whose call resolves to one of
/// them (or a member it overrides), and code that reads a holder of a target's type (a property or
/// accessor of it, through up to three wrappers) and writes the target's name. What that code runs
/// is the round's reach.
fn typed_round(
    v: &View,
    routes: &Routes,
    targets: &FxHashSet<u32>,
    seed_roots: &FxHashSet<u32>,
    prior: &FxHashSet<u32>,
) -> (FxHashSet<u32>, FxHashMap<u32, FxHashSet<u32>>) {
    let (flags, name, root) = (&v.flags, &v.name, &v.root);
    let mut overrides = Overrides::new(v);
    let mut keep: FxHashSet<u32> = FxHashSet::default();
    let mut by_type: FxHashMap<u32, (FxHashSet<u32>, u32)> = FxHashMap::default();
    for &t in targets {
        let mut members = vec![t];
        members.extend(overrides.of(t));
        for &m in &members {
            for &c in v.refby(m) {
                // Its own type, implementers (a supertype list) and their overrides reference it
                // structurally, not by a call.
                let ci = c as usize;
                let structural = root[ci] == root[m as usize]
                    || !is_holder(v, c)
                    || (name[ci] == name[m as usize]
                        && v.supertypes(root[ci])
                            .iter()
                            .any(|&s| root[s as usize] == root[m as usize]));
                if !structural && !prior.contains(&c) {
                    note(c, m, || "calls it (resolved)".into());
                    keep.insert(c);
                }
            }
            by_type
                .entry(root[m as usize])
                .or_insert_with(|| (FxHashSet::default(), t))
                .0
                .insert(name[m as usize]);
        }
    }
    for (ty, (words, via_member)) in by_type {
        let own: FxHashSet<u32> = v
            .supertypes(ty)
            .into_iter()
            .map(|t| root[t as usize])
            .collect();
        let mut holders: Vec<u32> = Vec::new();
        let mut seen: FxHashSet<u32> = FxHashSet::default();
        for m in ty..v.n as u32 {
            if m != ty && root[m as usize] != ty {
                break;
            }
            for &h in v.refby(m) {
                if is_holder(v, h)
                    && !own.contains(&root[h as usize])
                    && !words.contains(&name[h as usize])
                    && seen.insert(h)
                {
                    note(h, via_member, || "holds the interface".into());
                    holders.push(h);
                }
            }
        }
        let users: FxHashSet<u32> = words
            .iter()
            .flat_map(|&w| v.fam(Family::NameUsers, w).iter().copied())
            .collect();
        let mut users_in: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for &u in &users {
            users_in.entry(root[u as usize]).or_default().push(u);
        }
        let mut seers_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
        let mut opened: FxHashSet<u32> = FxHashSet::default();
        while let Some(h) = holders.pop() {
            let private = flags[h as usize] & N_PRIVATE != 0;
            let holder_root = root[h as usize];
            // Code in a class that holds the interface and writes the member's name may call it on
            // that holder (a class's own accessors are not always indexed by name).
            if !seed_roots.contains(&holder_root) && opened.insert(holder_root) {
                for &u in users_in.get(&holder_root).into_iter().flatten() {
                    if !prior.contains(&u) {
                        note(u, h, || {
                            format!("calls it on `{}`'s class", v.p.str(name[h as usize]))
                        });
                        keep.insert(u);
                    }
                }
            }
            let seers = seers_of.entry(holder_root).or_insert_with(|| {
                let mut seen: FxHashSet<u32> = FxHashSet::default();
                let mut frontier = vec![holder_root];
                for _ in 0..3 {
                    let mut next = Vec::new();
                    for r in frontier {
                        for &x in v.refby(r) {
                            let xr = root[x as usize];
                            if seen.insert(xr) {
                                next.push(xr);
                            }
                        }
                    }
                    frontier = next;
                }
                seen
            });
            for &w in v.fam(Family::NameUsers, name[h as usize]) {
                let wr = root[w as usize];
                if (private && wr != holder_root)
                    || (wr != holder_root && !seers.contains(&wr))
                    || !seen.insert(w)
                    || prior.contains(&w)
                {
                    continue;
                }
                if users.contains(&w) && !seed_roots.contains(&wr) {
                    note(w, h, || {
                        let ns: Vec<&str> = words.iter().map(|&x| v.p.str(x)).collect();
                        format!(
                            "reads `{}` and calls `{}`",
                            v.p.str(name[h as usize]),
                            ns.join("`/`")
                        )
                    });
                    keep.insert(w);
                } else if is_holder(v, w) && !words.contains(&name[w as usize]) {
                    note(w, h, || format!("holds `{}`", v.p.str(name[h as usize])));
                    holders.push(w);
                }
            }
        }
    }
    let mut by: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    let mut out: FxHashSet<u32> = FxHashSet::default();
    if !keep.is_empty() {
        let r = symbol_closure(v, routes, &keep, false, false);
        merge_by(&mut by, &r.dispatched_by);
        out.extend(r.nodes());
    }
    (out, by)
}

fn merge_by(into: &mut FxHashMap<u32, FxHashSet<u32>>, from: &FxHashMap<u32, FxHashSet<u32>>) {
    for (&t, by) in from {
        into.entry(t).or_default().extend(by.iter().copied());
    }
}

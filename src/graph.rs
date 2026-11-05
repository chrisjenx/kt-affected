//! Repo-wide symbol tables and the linker that turns declarations' name tokens into edges.
//!
//! Resolution is approximated the safe way: a simple name resolves against the file's own declarations, its explicit imports, its
//! wildcard-imported packages and its own package, and every candidate is taken. Members are
//! reachable only through a container the same declaration names (near), or through an implicit
//! receiver — the enclosing containers and their supertypes (far, a weak edge).
//!
//! Everything is u32 ids in flat arrays; `link` is a pure function of the symbol table and every
//! file's tokens, so an incremental update and a cold build produce the same graph.

use crate::parse::{
    Decl, FLAG_ANY_SEMANTICS, FLAG_INNER, FLAG_PRIVATE, FLAG_SEALED, FileRefs, Import, KIND_CLASS,
    KIND_CONSTRUCTOR, KIND_INIT, KIND_INTERFACE, KoinDefI, NONE, is_container_kind,
    is_inheritable_kind, is_type_kind,
};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

/// Compressed sparse rows: row `i` is `val[off[i]..off[i + 1]]`.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Csr {
    pub off: Vec<u32>,
    pub val: Vec<u32>,
}

impl Csr {
    pub fn get(&self, i: usize) -> &[u32] {
        if i + 1 >= self.off.len() {
            return &[];
        }
        &self.val[self.off[i] as usize..self.off[i + 1] as usize]
    }

    pub fn from_rows<I: IntoIterator<Item = R>, R: AsRef<[u32]>>(rows: I) -> Csr {
        let mut c = Csr {
            off: vec![0],
            val: Vec::new(),
        };
        for r in rows {
            c.val.extend_from_slice(r.as_ref());
            c.off.push(c.val.len() as u32);
        }
        c
    }

    /// Rows `0..n` built from (row, value) pairs; each row sorted and deduplicated.
    pub fn from_pairs(n: usize, pairs: &[(u32, u32)]) -> Csr {
        let mut sorted = pairs.to_vec();
        sorted.par_sort_unstable();
        sorted.dedup();
        let mut off = vec![0u32; n + 1];
        for &(r, _) in &sorted {
            off[r as usize + 1] += 1;
        }
        for i in 0..n {
            off[i + 1] += off[i];
        }
        Csr {
            off,
            val: sorted.into_iter().map(|p| p.1).collect(),
        }
    }
}

/// Repo-wide declaration tables. Node ids are `file_base[f] + local index`, files in path order.
#[derive(Default)]
pub struct Symbols {
    pub file_base: Vec<u32>,
    pub file_pkg: Vec<u32>,
    pub file_module: Vec<u32>,
    pub file_test: Vec<bool>,
    pub name: Vec<u32>,
    pub kind: Vec<u8>,
    pub flags: Vec<u8>,
    pub parent: Vec<i32>,
    pub file: Vec<u32>,
    pub lo: Vec<u32>,
    pub hi: Vec<u32>,
    pub supers: Csr,
    pub super_args: Csr,
    pub annotations: Csr,
}

impl Symbols {
    pub fn nodes(&self) -> usize {
        self.name.len()
    }

    pub fn file_nodes(&self, f: usize) -> std::ops::Range<u32> {
        let end = if f + 1 < self.file_base.len() {
            self.file_base[f + 1]
        } else {
            self.name.len() as u32
        };
        self.file_base[f]..end
    }

    pub fn push_file(&mut self, pkg: u32, module: u32, test: bool, decls: &[Decl]) {
        let f = self.file_base.len() as u32;
        let base = self.name.len() as u32;
        self.file_base.push(base);
        self.file_pkg.push(pkg);
        self.file_module.push(module);
        self.file_test.push(test);
        for d in decls {
            self.name.push(d.name);
            self.kind.push(d.kind);
            self.flags.push(d.flags);
            self.parent.push(if d.parent < 0 {
                -1
            } else {
                base as i32 + d.parent
            });
            self.file.push(f);
            self.lo.push(d.lo);
            self.hi.push(d.hi);
            push_row(&mut self.supers, &d.supers);
            push_row(&mut self.super_args, &d.super_args);
            push_row(&mut self.annotations, &d.annotations);
        }
    }
}

fn push_row(c: &mut Csr, row: &[u32]) {
    if c.off.is_empty() {
        c.off.push(0);
    }
    c.val.extend_from_slice(row);
    c.off.push(c.val.len() as u32);
}

/// Module visibility: module index -> bitset of modules it may reference. `None` sees everything.
pub struct Visibility {
    pub bits: Vec<Option<Vec<u64>>>,
}

impl Visibility {
    pub fn sees(&self, from: u32, to: u32) -> bool {
        if from == NONE || to == NONE {
            return true;
        }
        match &self.bits[from as usize] {
            None => true,
            Some(b) => b[to as usize / 64] >> (to % 64) & 1 == 1,
        }
    }
}

// Edge reasons; a pair's bits are the union of every reason that linked it.
pub const E_IMPL: u8 = 1;
pub const E_SUPER: u8 = 2;
pub const E_STRONG: u8 = 4;

// Node flags of the linked graph.
#[allow(dead_code)] // reserved bit; the layout is shared with the cache format
pub const N_RIDES_ON_PARENT: u8 = 1;
pub const N_NO_WALK: u8 = 2;
pub const N_IN_TEST: u8 = 4;
pub const N_PRIVATE: u8 = 8;
pub const N_MARKER: u8 = 16;
pub const N_ANY_MEMBERS: u8 = 32;
/// A provided interface: a definition asking for one is handed a binding.
pub const N_ASKED: u8 = 64;

/// Names Kotlin calls without their being written: `a + b`, `x[i]`, `by lazy`, destructuring.
pub const OPERATOR_NAMES: &[&str] = &[
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
const ANY_MEMBER_NAMES: &[&str] = &["equals", "hashCode", "toString"];

/// Every family of the linked graph.
#[derive(Default)]
pub struct Graph {
    /// target -> referrers
    pub refby: Csr,
    /// referrer -> its supertypes and their inherited members
    pub declares: Csr,
    /// declaration -> supertypes of `object : X` expressions it writes
    pub anon: Csr,
    /// target -> referrers reaching it only weakly (subtype, implicit receiver, nested type)
    pub vblock: Csr,
    /// target -> referrers linked to it only through a type argument's member layer
    pub targ_only: Csr,
    /// referrer -> the type arguments its file resolved through
    pub targ_via: Csr,
    /// top-level extension -> its receiver types
    pub ext_recv: Csr,
    /// interned member name -> declarations whose own code writes it
    pub name_users: Csr,
    /// provided declaration -> [module, walk length, walk...]*
    pub provides: Csr,
    /// provided class -> what constructing it runs; [NONE] when anything it names
    pub builds: Csr,
    /// provided declaration -> modules whose edge to it `provides` replaces
    pub koin_routed: Csr,
    /// declaration -> type arguments of its supertypes (input of the type-argument layer)
    pub type_args: Csr,
    pub flags: Vec<u8>,
}

/// Repo-wide inputs of the type-argument member layer.
#[derive(Default)]
pub(crate) struct TargPrep {
    users: FxHashSet<u32>,
    user_names: FxHashSet<u32>,
    through: FxHashMap<u32, Vec<u32>>,
}

/// Repo-wide inputs of a file's assembly.
pub(crate) struct Prep {
    targ: TargPrep,
    declares: Csr,
    member_names: FxHashSet<u32>,
}

/// One file's (row, value) pairs of the families whose pairs its own declarations make:
/// `refby`/`vblock`/`targ_only` are (target, source), `via` (source, argument), `users` (name, source).
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct Contrib {
    pub refby: Vec<(u32, u32)>,
    pub vblock: Vec<(u32, u32)>,
    pub targ_only: Vec<(u32, u32)>,
    pub via: Vec<(u32, u32)>,
    pub users: Vec<(u32, u32)>,
}

pub struct Linker<'a> {
    pub sym: &'a Symbols,
    pub vis: &'a Visibility,
    pub refs: &'a [FileRefs],
    children: Csr,
    reachable: Vec<bool>,
    root: Vec<u32>,
    /// (package, name) -> reachable top-level declarations
    pkg_index: FxHashMap<(u32, u32), Vec<u32>>,
    /// package -> reachable top-level declarations
    pkg_roots: FxHashMap<u32, Vec<u32>>,
    /// a package's segments -> the package, for fully qualified names written in code
    pkg_by_segs: FxHashMap<Vec<u32>, u32>,
    /// node -> (name, node) of itself and its reachable descendants, sorted by name
    under: Csr,
    /// node -> number of distinct names under it
    under_names: Vec<u32>,
    operator_ids: FxHashSet<u32>,
}

struct Scope<'l, 'a> {
    l: &'l Linker<'a>,
    file: usize,
    from_test: bool,
    local: FxHashMap<u32, Vec<u32>>,
    imported: FxHashMap<u32, Vec<u32>>,
    probed: Vec<u32>,
}

/// One file's link output.
#[derive(Clone, Default)]
pub(crate) struct FileOut {
    /// (target, source, bits)
    edges: Vec<(u32, u32, u8)>,
    declares: Vec<(u32, u32)>,
    anon: Vec<(u32, u32)>,
    type_args: Vec<(u32, u32)>,
    ext_recv: Vec<(u32, u32)>,
    /// nested non-inner types not (strongly) referencing their parent through their own code
    nested: Vec<u32>,
    unseen_super: Vec<u32>,
    class_params: Vec<(u32, Vec<u32>)>,
    class_builds: Vec<(u32, Option<Vec<u32>>)>,
    /// (target, module) edges a Koin module only registers, never uses
    koin_discard: Vec<(u32, u32)>,
    /// (target, module, walk)
    koin_provides: Vec<(u32, u32, Vec<u32>)>,
    /// (module, class written by an untyped definition, walk)
    koin_untyped: Vec<(u32, u32, Vec<u32>)>,
}

impl FileOut {
    /// Whether two link results of one file agree on everything that reaches past the file's own
    /// pairs: supertypes, type arguments, receivers, construction and Koin wiring.
    pub(crate) fn same_reach_outside(&self, o: &FileOut) -> bool {
        fn sorted<T: Clone + Ord>(v: &[T]) -> Vec<T> {
            let mut v = v.to_vec();
            v.sort();
            v
        }
        sorted(&self.declares) == sorted(&o.declares)
            && sorted(&self.anon) == sorted(&o.anon)
            && sorted(&self.type_args) == sorted(&o.type_args)
            && sorted(&self.ext_recv) == sorted(&o.ext_recv)
            && sorted(&self.unseen_super) == sorted(&o.unseen_super)
            && sorted(&self.class_params) == sorted(&o.class_params)
            && sorted(&self.class_builds) == sorted(&o.class_builds)
            && sorted(&self.koin_discard) == sorted(&o.koin_discard)
            && sorted(&self.koin_provides) == sorted(&o.koin_provides)
            && sorted(&self.koin_untyped) == sorted(&o.koin_untyped)
    }
}

impl<'a> Linker<'a> {
    pub fn new(
        sym: &'a Symbols,
        vis: &'a Visibility,
        refs: &'a [FileRefs],
        strs: &[Box<str>],
        interned: impl Fn(&str) -> Option<u32>,
    ) -> Self {
        let n = sym.nodes();
        let pairs: Vec<(u32, u32)> = (0..n)
            .filter(|&i| sym.parent[i] >= 0)
            .map(|i| (sym.parent[i] as u32, i as u32))
            .collect();
        // Children in declaration order (pairs are generated in node order, a stable fill keeps it).
        let mut count = vec![0u32; n + 1];
        for &(r, _) in &pairs {
            count[r as usize + 1] += 1;
        }
        for i in 0..n {
            count[i + 1] += count[i];
        }
        let mut fill = count.clone();
        let mut val = vec![0u32; pairs.len()];
        for &(r, v) in &pairs {
            val[fill[r as usize] as usize] = v;
            fill[r as usize] += 1;
        }
        let children = Csr { off: count, val };
        let mut reachable = vec![false; n];
        let mut root = vec![0u32; n];
        for i in 0..n {
            let p = sym.parent[i];
            reachable[i] = sym.flags[i] & FLAG_PRIVATE == 0 && (p < 0 || reachable[p as usize]);
            root[i] = if p < 0 { i as u32 } else { root[p as usize] };
        }
        let mut pkg_index: FxHashMap<(u32, u32), Vec<u32>> = FxHashMap::default();
        let mut pkg_roots: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for i in 0..n {
            if sym.parent[i] < 0 && reachable[i] {
                let pkg = sym.file_pkg[sym.file[i] as usize];
                pkg_index
                    .entry((pkg, sym.name[i]))
                    .or_default()
                    .push(i as u32);
                pkg_roots.entry(pkg).or_default().push(i as u32);
            }
        }
        let pkg_by_segs: FxHashMap<Vec<u32>, u32> = pkg_roots
            .keys()
            .filter_map(|&pkg| {
                let text = strs.get(pkg as usize)?;
                let segs: Option<Vec<u32>> = text.split('.').map(&interned).collect();
                Some((segs?, pkg))
            })
            .collect();
        // names_under for every node: itself plus reachable descendants.
        let rows: Vec<Vec<u32>> = (0..n)
            .into_par_iter()
            .map(|i| {
                let mut out: Vec<(u32, u32)> = vec![(sym.name[i], i as u32)];
                let mut stack = vec![i as u32];
                while let Some(c) = stack.pop() {
                    let (a, b) = (
                        children.off[c as usize] as usize,
                        children.off[c as usize + 1] as usize,
                    );
                    for &k in &children.val[a..b] {
                        if reachable[k as usize] {
                            out.push((sym.name[k as usize], k));
                            stack.push(k);
                        }
                    }
                }
                out.sort_unstable();
                out.into_iter().flat_map(|(a, b)| [a, b]).collect()
            })
            .collect();
        let under_names = rows
            .iter()
            .map(|r| {
                let mut d = 0u32;
                let mut last = NONE;
                for p in r.chunks_exact(2) {
                    if p[0] != last {
                        d += 1;
                        last = p[0];
                    }
                }
                d
            })
            .collect();
        let under = Csr::from_rows(rows);
        let operator_ids = OPERATOR_NAMES.iter().filter_map(|s| interned(s)).collect();
        Linker {
            sym,
            vis,
            refs,
            children,
            reachable,
            root,
            pkg_index,
            pkg_roots,
            pkg_by_segs,
            under,
            under_names,
            operator_ids,
        }
    }

    fn kids(&self, n: u32) -> &[u32] {
        self.children.get(n as usize)
    }

    fn under_named(&self, n: u32, name: u32) -> impl Iterator<Item = u32> + '_ {
        let row = self.under.get(n as usize);
        let pairs = row.len() / 2;
        // lower bound on name
        let (mut lo, mut hi) = (0usize, pairs);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if row[mid * 2] < name {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        (lo..pairs)
            .take_while(move |&k| row[k * 2] == name)
            .map(move |k| row[k * 2 + 1])
    }

    /// The node plus what a subtype gets from it: its members, not its nested types.
    fn inherited(&self, n: u32) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(n).chain(
            self.kids(n)
                .iter()
                .copied()
                .filter(|&k| is_inheritable_kind(self.sym.kind[k as usize])),
        )
    }

    fn descendants(&self, n: u32, out: &mut Vec<u32>) {
        out.push(n);
        let mut stack = vec![n];
        while let Some(c) = stack.pop() {
            for &k in self.kids(c) {
                out.push(k);
                stack.push(k);
            }
        }
    }

    fn resolve_fqn(&self, imp: &Import) -> Vec<u32> {
        let segs = &imp.segs;
        for split in (1..segs.len()).rev() {
            let Some(hit) = self.pkg_index.get(&(imp.prefixes[split - 1], segs[split])) else {
                continue;
            };
            let mut cur: Vec<u32> = hit.clone();
            for &seg in &segs[split + 1..] {
                let next: Vec<u32> = cur
                    .iter()
                    .flat_map(|&n| self.kids(n).iter().copied())
                    .filter(|&k| self.sym.name[k as usize] == seg)
                    .collect();
                if next.is_empty() {
                    return cur; // member of a member we cannot see: widen here
                }
                cur = next;
            }
            return cur;
        }
        Vec::new()
    }

    /// A fully qualified chain (`[demo, model, Money]`): the declarations its longest package
    /// prefix and the names after it select.
    fn resolve_qualified(&self, chain: &[u32]) -> Vec<u32> {
        for split in (1..chain.len()).rev() {
            let Some(&pkg) = self.pkg_by_segs.get(&chain[..split]) else {
                continue;
            };
            let Some(hit) = self.pkg_index.get(&(pkg, chain[split])) else {
                continue;
            };
            let mut cur: Vec<u32> = hit.clone();
            for &seg in &chain[split + 1..] {
                let next: Vec<u32> = cur
                    .iter()
                    .flat_map(|&n| self.kids(n).iter().copied())
                    .filter(|&k| self.sym.name[k as usize] == seg)
                    .collect();
                if next.is_empty() {
                    break;
                }
                cur = next;
            }
            return cur;
        }
        Vec::new()
    }

    fn scope(&self, f: usize, refs: &FileRefs) -> Scope<'_, 'a> {
        let s = self.sym;
        let mut local: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for n in s.file_nodes(f) {
            local.entry(s.name[n as usize]).or_default().push(n);
        }
        let mut imported: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        let mut probed = Vec::new();
        for imp in &refs.imports {
            if imp.star {
                if let Some(&pkg) = imp.prefixes.last()
                    && self.pkg_roots.contains_key(&pkg)
                {
                    probed.push(pkg);
                }
                continue;
            }
            for n in self.resolve_fqn(imp) {
                let key = if imp.alias != NONE {
                    imp.alias
                } else {
                    s.name[n as usize]
                };
                imported.entry(key).or_default().push(n);
            }
        }
        let own = s.file_pkg[f];
        if self.pkg_roots.contains_key(&own) {
            probed.push(own);
        }
        Scope {
            l: self,
            file: f,
            from_test: s.file_test[f],
            local,
            imported,
            probed,
        }
    }

    /// Link one file from its own tokens: per-file edges and facts, as the reference linker's per-file loop.
    pub(crate) fn link_file(&self, f: usize, refs: &FileRefs) -> FileOut {
        let s = self.sym;
        let range = s.file_nodes(f);
        let base = range.start;
        let mut out = FileOut::default();
        if range.is_empty() {
            return out;
        }
        let scope = self.scope(f, refs);
        let mut bits: FxHashMap<(u32, u32), u8> = FxHashMap::default();
        let add = |bits: &mut FxHashMap<(u32, u32), u8>, src: u32, t: u32, b: u8| {
            if t != src {
                *bits.entry((t, src)).or_insert(0) |= b;
            }
        };
        let tokens = |n: u32| -> &[u32] {
            refs.refs
                .get((n - base) as usize)
                .map(|v| v.as_slice())
                .unwrap_or(&[])
        };
        let mut file_tokens: FxHashSet<u32> = FxHashSet::default();
        for n in range.clone() {
            file_tokens.extend(tokens(n).iter().copied());
        }
        // Implicit-receiver layers: each container's supertypes, the parent, the parent's layers.
        let mut enclosing: Vec<Vec<u32>> = Vec::with_capacity(range.len());
        for n in range.clone() {
            let mut layer: Vec<u32> = Vec::new();
            if is_container_kind(s.kind[n as usize]) {
                for &name in s.supers.get(n as usize) {
                    layer.extend(scope.resolve(name));
                }
            }
            let p = s.parent[n as usize];
            if p >= 0 {
                layer.push(p as u32);
                layer.extend(enclosing[(p as u32 - base) as usize].iter().copied());
            }
            enclosing.push(layer);
        }
        let params_at: FxHashMap<u32, &Vec<u32>> =
            refs.params.iter().map(|(i, v)| (*i, v)).collect();
        let builds_at: FxHashMap<u32, &Option<Vec<u32>>> =
            refs.builds.iter().map(|(i, v)| (*i, v)).collect();
        let koin_at: FxHashMap<u32, &crate::parse::KoinModuleI> =
            refs.koin.iter().map(|m| (m.decl, m)).collect();
        for n in range.clone() {
            let i = (n - base) as usize;
            let (mut near, mut far) = scope.resolve_split(tokens(n), &enclosing[i]);
            if let Some(q) = refs.qualified.get(i).filter(|q| !q.is_empty()) {
                let mut hits: Vec<u32> = Vec::new();
                for chain in q.split(|&x| x == NONE).filter(|c| c.len() >= 2) {
                    for t in self.resolve_qualified(chain) {
                        hits.push(t);
                        if self.under_names[t as usize] > 1 {
                            for &name in tokens(n) {
                                hits.extend(self.under_named(t, name));
                            }
                        }
                    }
                }
                if !hits.is_empty() {
                    near.extend(scope.keep(hits));
                    far.retain(|x| !near.contains(x));
                }
            }
            // A private class member is visible only inside its class, so never from another
            // top-level declaration of the file (the file-wide name table offers it there).
            let foreign_private = |t: &u32| {
                let ti = *t as usize;
                s.flags[ti] & FLAG_PRIVATE != 0
                    && s.parent[ti] >= 0
                    && s.file[ti] == s.file[n as usize]
                    && self.root[ti] != self.root[n as usize]
            };
            near.retain(|t| !foreign_private(t));
            far.retain(|t| !foreign_private(t));
            for &t in &near {
                add(&mut bits, n, t, E_STRONG);
            }
            for &t in &far {
                add(&mut bits, n, t, E_IMPL);
            }
            if let Some(names) = params_at.get(&(i as u32)) {
                let (pn, pf) = scope.resolve_split(names, &enclosing[i]);
                let v: Vec<u32> = pn
                    .into_iter()
                    .chain(pf)
                    .filter(|&t| is_type_kind(s.kind[t as usize]))
                    .collect();
                out.class_params.push((n, v));
            }
            if let Some(names) = builds_at.get(&(i as u32)) {
                match names {
                    None => out.class_builds.push((n, None)),
                    Some(v) if !v.is_empty() => {
                        let (bn, bf) = scope.resolve_split(v, &enclosing[i]);
                        out.class_builds
                            .push((n, Some(bn.into_iter().chain(bf).collect())));
                    }
                    Some(_) => {}
                }
            }
            if let Some(module) = koin_at.get(&(i as u32)) {
                let targets: Vec<u32> = near.iter().chain(far.iter()).copied().collect();
                self.koin_route(&scope, n, module, &targets, &refs.imports, &mut out);
            }
            for (into, names) in [
                (0, s.supers.get(n as usize)),
                (1, refs.anon.get(i).map(|v| v.as_slice()).unwrap_or(&[])),
            ] {
                for &name in names {
                    let supers = scope.resolve_supertype(name);
                    for &t in &supers {
                        add(&mut bits, n, t, E_SUPER);
                        if t != n {
                            if into == 0 {
                                out.declares.push((n, t))
                            } else {
                                out.anon.push((n, t))
                            }
                        }
                    }
                    if into == 0 && !supers.iter().any(|&t| is_type_kind(s.kind[t as usize])) {
                        out.unseen_super.push(n);
                    }
                }
            }
            for &name in s.super_args.get(n as usize) {
                for t in scope.resolve(name) {
                    if is_type_kind(s.kind[t as usize]) {
                        out.type_args.push((n, t));
                    }
                }
            }
            if let Some(path) = refs.receiver.get(i).filter(|p| !p.is_empty()) {
                for t in scope.resolve_type_path(path) {
                    out.ext_recv.push((n, t));
                }
            }
            let p = s.parent[n as usize];
            if p >= 0 {
                let p = p as u32;
                let nested = is_type_kind(s.kind[n as usize])
                    && s.flags[n as usize] & FLAG_INNER == 0
                    && bits.get(&(p, n)).map(|b| b & E_STRONG == 0).unwrap_or(true);
                add(&mut bits, n, p, E_STRONG);
                if nested {
                    out.nested.push(n);
                }
                if matches!(s.kind[n as usize], KIND_INIT | KIND_CONSTRUCTOR) {
                    add(&mut bits, p, n, E_STRONG);
                }
            }
        }
        // Imports no declaration body accounts for widen from the leaves.
        let carriers: Vec<u32> = range.clone().filter(|&n| self.kids(n).is_empty()).collect();
        let in_code: FxHashSet<u32> = refs.in_code.iter().copied().collect();
        let mut discard_nested: FxHashSet<u32> = FxHashSet::default();
        for imp in &refs.imports {
            let targets = scope.unattributed_import(imp, &file_tokens, &in_code);
            if targets.is_empty() {
                continue;
            }
            for &c in &carriers {
                let pc = s.parent[c as usize];
                for &t in &targets {
                    add(&mut bits, c, t, E_STRONG);
                    if pc >= 0 && t == pc as u32 {
                        discard_nested.insert(c);
                    }
                }
            }
        }
        out.nested.retain(|n| !discard_nested.contains(n));
        out.edges = bits.into_iter().map(|((t, src), b)| (t, src, b)).collect();
        out
    }

    /// Which of a Koin module's targets it uses, and which it only registers under a key.
    fn koin_route(
        &self,
        scope: &Scope,
        module: u32,
        m: &crate::parse::KoinModuleI,
        targets: &[u32],
        imports: &[Import],
        out: &mut FileOut,
    ) {
        let s = self.sym;
        // name -> (owner, local name) per explicit `import ….Owner.name [as local]`; owners of `.*` imports.
        let mut imported: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
        let mut star: FxHashSet<u32> = FxHashSet::default();
        for imp in imports {
            let k = imp.segs.len();
            if imp.star {
                if let Some(&last) = imp.segs.last() {
                    star.insert(last);
                }
            } else if k > 1 {
                let local = if imp.alias != NONE {
                    imp.alias
                } else {
                    imp.segs[k - 1]
                };
                imported
                    .entry(imp.segs[k - 1])
                    .or_default()
                    .push((imp.segs[k - 2], local));
            }
        }
        struct Def<'d> {
            d: &'d KoinDefI,
            names: FxHashSet<u32>,
            dotted: FxHashSet<u32>,
        }
        fn mk(d: &KoinDefI) -> Def<'_> {
            Def {
                d,
                names: d.names.iter().copied().collect(),
                dotted: d.dotted.iter().copied().collect(),
            }
        }
        let mut routable: Vec<(Def, Vec<u32>)> = Vec::new();
        let mut kept: Vec<Def> = vec![mk(&m.rest)];
        for d in &m.defs {
            let mut walk: Vec<u32> = Vec::new();
            let mut ok = !d.keys.is_empty() && !d.eager;
            if ok {
                for &key in &d.keys {
                    let hits = scope.resolve(key);
                    if hits.is_empty() || hits.iter().any(|&h| !is_type_kind(s.kind[h as usize])) {
                        ok = false;
                        break;
                    }
                    for h in hits {
                        walk.extend(self.inherited(h));
                    }
                }
            }
            if ok {
                walk.sort_unstable();
                walk.dedup();
                routable.push((mk(d), walk));
            } else {
                kept.push(mk(d));
            }
        }
        let mut enclosing_module: FxHashSet<u32> = FxHashSet::default();
        let mut p = s.parent[module as usize];
        while p >= 0 {
            enclosing_module.insert(p as u32);
            p = s.parent[p as usize];
        }
        let uses = |d: &Def, t: u32, owners: &FxHashSet<u32>| -> bool {
            let name = s.name[t as usize];
            if imported
                .get(&name)
                .map(|v| {
                    v.iter()
                        .any(|(o, l)| owners.contains(o) && d.names.contains(l))
                })
                .unwrap_or(false)
            {
                return true;
            }
            if !d.names.contains(&name) {
                return false;
            }
            let pt = s.parent[t as usize];
            if pt < 0 || enclosing_module.contains(&(pt as u32)) || is_type_kind(s.kind[t as usize])
            {
                return true;
            }
            if owners.iter().any(|o| star.contains(o)) {
                return true;
            }
            if d.dotted.contains(&name)
                || d.d
                    .qualified
                    .chunks_exact(2)
                    .any(|q| q[1] == name && owners.contains(&q[0]))
            {
                return true;
            }
            d.d.receivers && owners.iter().any(|o| d.names.contains(o))
        };
        for &t in targets {
            if matches!(s.kind[t as usize], KIND_INIT | KIND_CONSTRUCTOR) {
                out.koin_discard.push((t, module));
                continue;
            }
            let mut owners: FxHashSet<u32> = FxHashSet::default();
            let mut p = s.parent[t as usize];
            while p >= 0 {
                owners.insert(s.name[p as usize]);
                p = s.parent[p as usize];
            }
            if kept.iter().any(|d| uses(d, t, &owners)) {
                continue;
            }
            let walk: Vec<u32> = routable
                .iter()
                .filter(|(d, _)| uses(d, t, &owners))
                .flat_map(|(_, w)| w.iter().copied())
                .collect();
            if routable.iter().any(|(d, _)| uses(d, t, &owners)) {
                out.koin_provides.push((t, module, walk));
            } else {
                out.koin_discard.push((t, module));
            }
        }
        // `X(get())` and `singleOf(::X)` ask for X's parameter types without writing them.
        for (d, walk) in &routable {
            if !d.d.untyped {
                continue;
            }
            for &t in targets {
                if s.kind[t as usize] == KIND_CLASS && d.names.contains(&s.name[t as usize]) {
                    out.koin_untyped.push((module, t, walk.clone()));
                }
            }
        }
    }

    /// Repo-wide inputs of every file's assembly. They move only when a declaration, a supertype,
    /// a type argument or Koin wiring moves.
    pub(crate) fn prep(
        &self,
        declares: &Csr,
        type_args: &Csr,
        interned: impl Fn(&str) -> Option<u32>,
    ) -> Prep {
        let s = self.sym;
        let implicit: FxHashSet<u32> = OPERATOR_NAMES
            .iter()
            .chain(["toString", "hashCode"].iter())
            .filter_map(|w| interned(w))
            .collect();
        let member_names: FxHashSet<u32> = (0..s.nodes())
            .filter(|&i| s.parent[i] >= 0 && self.reachable[i])
            .map(|i| s.name[i])
            .filter(|w| !implicit.contains(w))
            .collect();
        Prep {
            targ: self.targ_prep(declares, type_args),
            declares: declares.clone(),
            member_names,
        }
    }

    /// One file's share of the families whose pairs its own declarations make. No step reads another
    /// file's edges, so the graph is the union of these and one file's share can be swapped alone.
    pub(crate) fn assemble(&self, f: usize, refs: &FileRefs, out: FileOut, prep: &Prep) -> Contrib {
        let s = self.sym;
        let FileOut {
            mut edges,
            nested,
            koin_discard,
            ..
        } = out;
        let mut nested: FxHashSet<u32> = nested.into_iter().collect();
        edges.sort_unstable_by_key(|e| (e.0, e.1));
        edges.dedup_by(|b, a| {
            if (a.0, a.1) == (b.0, b.1) {
                a.2 |= b.2;
                true
            } else {
                false
            }
        });
        let find = |v: &[(u32, u32, u8)], t: u32, src: u32| {
            v.binary_search_by_key(&(t, src), |e| (e.0, e.1)).ok()
        };
        let mut c = Contrib::default();
        // ---- type-argument member layers
        let mut extra: FxHashMap<(u32, u32), u8> = FxHashMap::default();
        if let Some((args, links)) = self.targ_file(f, refs, prep) {
            for (src, targets) in links {
                let mut fresh = false;
                for t in targets {
                    if t == src {
                        continue;
                    }
                    let e = match find(&edges, t, src) {
                        Some(i) => &mut edges[i].2,
                        None => extra.entry((t, src)).or_insert(0),
                    };
                    if *e & E_STRONG == 0 {
                        c.targ_only.push((t, src));
                        fresh = true;
                    }
                    *e |= E_STRONG;
                    if s.parent[src as usize] >= 0 && t == s.parent[src as usize] as u32 {
                        nested.remove(&src);
                    }
                }
                if fresh {
                    c.via.extend(args.iter().map(|&v| (src, v)));
                }
            }
        }
        // Extras are new keys (existing ones were updated in place): a linear merge keeps order.
        let mut add: Vec<(u32, u32, u8)> =
            extra.into_iter().map(|((t, src), b)| (t, src, b)).collect();
        add.sort_unstable_by_key(|e| (e.0, e.1));
        if !add.is_empty() {
            let old = std::mem::take(&mut edges);
            edges.reserve(old.len() + add.len());
            let (mut i, mut j) = (0, 0);
            while i < old.len() || j < add.len() {
                if j == add.len() || (i < old.len() && (old[i].0, old[i].1) < (add[j].0, add[j].1))
                {
                    edges.push(old[i]);
                    i += 1;
                } else {
                    edges.push(add[j]);
                    j += 1;
                }
            }
        }
        // ---- Koin: a target a module only registers carries nothing; a routed edge stays,
        // since it still carries an edit to the declaration itself.
        for &(t, m) in &koin_discard {
            if let Some(i) = find(&edges, t, m) {
                edges[i].2 = 0;
            }
        }
        c.refby.reserve(edges.len());
        for &(t, src, b) in &edges {
            if b == 0 {
                continue;
            }
            c.refby.push((t, src));
            if b & (E_IMPL | E_SUPER) != 0 && (b & E_SUPER != 0 || b & E_STRONG == 0) {
                c.vblock.push((t, src));
            }
        }
        for &k in &nested {
            c.vblock.push((s.parent[k as usize] as u32, k));
        }
        // ---- member names written by each declaration
        let base = s.file_base[f];
        for node in s.file_nodes(f) {
            for &w in refs.refs.get((node - base) as usize).into_iter().flatten() {
                if prep.member_names.contains(&w) {
                    c.users.push((w, node));
                }
            }
        }
        c
    }

    /// Link the whole repo.
    pub fn link(&self, interned: impl Fn(&str) -> Option<u32> + Sync) -> Graph {
        let clock = std::time::Instant::now();
        let prof = std::env::var_os("KT_AFFECTED_PROFILE").is_some();
        let lap = |what: &str| {
            if prof {
                eprintln!("  link {what}: {}ms", clock.elapsed().as_millis());
            }
        };
        let s = self.sym;
        let n = s.nodes();
        let files = s.file_base.len();
        let mut outs: Vec<FileOut> = (0..files)
            .into_par_iter()
            .map(|f| self.link_file(f, &self.refs[f]))
            .collect();
        lap("files");

        let mut declares_pairs = Vec::new();
        let mut anon_pairs = Vec::new();
        let mut targ_pairs = Vec::new();
        let mut recv_pairs = Vec::new();
        let mut unseen: FxHashSet<u32> = FxHashSet::default();
        let mut class_params: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        let mut class_builds: FxHashMap<u32, Option<Vec<u32>>> = FxHashMap::default();
        let mut provides: FxHashMap<
            u32,
            std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>>,
        > = FxHashMap::default();
        let mut koin_routed: Vec<(u32, u32)> = Vec::new();
        let mut koin_untyped = Vec::new();
        for o in &mut outs {
            class_params.extend(o.class_params.drain(..));
            class_builds.extend(o.class_builds.drain(..));
            for (t, m, walk) in o.koin_provides.drain(..) {
                provides
                    .entry(t)
                    .or_default()
                    .entry(m)
                    .or_default()
                    .extend(walk);
                koin_routed.push((t, m));
            }
            koin_untyped.append(&mut o.koin_untyped);
            declares_pairs.extend_from_slice(&o.declares);
            anon_pairs.extend_from_slice(&o.anon);
            targ_pairs.extend_from_slice(&o.type_args);
            recv_pairs.extend_from_slice(&o.ext_recv);
            unseen.extend(o.unseen_super.iter().copied());
        }
        let declares = Csr::from_pairs(n, &declares_pairs);
        let type_args = Csr::from_pairs(n, &targ_pairs);
        let prep = self.prep(&declares, &type_args, &interned);
        lap("prep");

        let contribs: Vec<Contrib> = outs
            .into_par_iter()
            .enumerate()
            .map(|(f, o)| self.assemble(f, &self.refs[f], o, &prep))
            .collect();
        lap("assemble");
        let mut ref_pairs = Vec::with_capacity(contribs.iter().map(|c| c.refby.len()).sum());
        let mut block_pairs = Vec::new();
        let mut targ_only_pairs = Vec::new();
        let mut via_pairs = Vec::new();
        let mut user_pairs = Vec::new();
        for c in contribs {
            ref_pairs.extend(c.refby);
            block_pairs.extend(c.vblock);
            targ_only_pairs.extend(c.targ_only);
            via_pairs.extend(c.via);
            user_pairs.extend(c.users);
        }
        let n_strings = user_pairs
            .iter()
            .map(|p| p.0 as usize + 1)
            .max()
            .unwrap_or(0);

        for (m, t, walk) in koin_untyped {
            for &p in class_params.get(&t).into_iter().flatten() {
                provides
                    .entry(p)
                    .or_default()
                    .entry(m)
                    .or_default()
                    .extend(walk.iter().copied());
            }
        }
        let mut provides_rows: Vec<(u32, Vec<u32>)> = Vec::new();
        let mut build_rows: Vec<(u32, Vec<u32>)> = Vec::new();
        let mut asked: FxHashSet<u32> = FxHashSet::default();
        for (&t, by_module) in &provides {
            let mut row = Vec::new();
            for (&m, walk) in by_module {
                row.push(m);
                row.push(walk.len() as u32);
                row.extend(walk.iter().copied());
            }
            provides_rows.push((t, row));
            match s.kind[t as usize] {
                KIND_INTERFACE => {
                    asked.insert(t);
                }
                KIND_CLASS => {
                    let row = match class_builds.get(&t) {
                        Some(None) => vec![NONE],
                        other => {
                            let mut v: Vec<u32> = other.and_then(|o| o.clone()).unwrap_or_default();
                            v.extend(self.kids(t).iter().copied().filter(|&k| {
                                matches!(s.kind[k as usize], KIND_INIT | KIND_CONSTRUCTOR)
                            }));
                            v.sort_unstable();
                            v.dedup();
                            v
                        }
                    };
                    build_rows.push((t, row));
                }
                _ => {}
            }
        }
        let rows_csr = |n: usize, mut rows: Vec<(u32, Vec<u32>)>| -> Csr {
            rows.sort_by_key(|r| r.0);
            let mut c = Csr {
                off: vec![0; n + 1],
                val: Vec::new(),
            };
            let mut it = rows.into_iter().peekable();
            for i in 0..n {
                if it.peek().map(|r| r.0 as usize == i).unwrap_or(false) {
                    c.val.extend(it.next().unwrap().1);
                }
                c.off[i + 1] = c.val.len() as u32;
            }
            c
        };
        lap("koin");

        // ---- flags
        let markers = self.markers(&declares, &unseen);
        let flags: Vec<u8> = (0..n)
            .map(|i| {
                let in_test = s.file_test[s.file[i] as usize];
                let mut fl = 0u8;
                if in_test {
                    fl |= N_IN_TEST;
                    if !self.reachable[i] {
                        fl |= N_NO_WALK;
                    }
                }
                if !self.reachable[i] {
                    fl |= N_PRIVATE;
                }
                if markers[i] {
                    fl |= N_MARKER;
                }
                if asked.contains(&(i as u32)) {
                    fl |= N_ASKED;
                }
                if is_type_kind(s.kind[i])
                    && (s.flags[i] & FLAG_ANY_SEMANTICS != 0
                        || self.kids(i as u32).iter().any(|&k| {
                            ANY_MEMBER_NAMES
                                .iter()
                                .any(|w| interned(w) == Some(s.name[k as usize]))
                        }))
                {
                    fl |= N_ANY_MEMBERS;
                }
                fl
            })
            .collect();

        lap("flags");
        let pair_sets: [(usize, &[(u32, u32)]); 8] = [
            (n, &ref_pairs),
            (n, &anon_pairs),
            (n, &block_pairs),
            (n, &targ_only_pairs),
            (n, &via_pairs),
            (n, &recv_pairs),
            (n_strings, &user_pairs),
            (n, &koin_routed),
        ];
        let mut csrs: Vec<Csr> = pair_sets
            .par_iter()
            .map(|&(rows, p)| Csr::from_pairs(rows, p))
            .collect();
        lap("csr");
        let mut take = || csrs.remove(0);
        Graph {
            refby: take(),
            declares,
            anon: take(),
            vblock: take(),
            targ_only: take(),
            targ_via: take(),
            ext_recv: take(),
            name_users: take(),
            koin_routed: take(),
            provides: rows_csr(n, provides_rows),
            builds: rows_csr(n, build_rows),
            type_args,
            flags,
        }
    }

    /// Types with nothing to dispatch: no members, not sealed, only marker supertypes in the repo.
    fn markers(&self, declares: &Csr, unseen: &FxHashSet<u32>) -> Vec<bool> {
        let s = self.sym;
        let n = s.nodes();
        let mut memo: Vec<u8> = vec![0; n]; // 0 unknown, 1 in progress / false, 2 true
        let mut done = vec![false; n];
        fn is_marker(
            l: &Linker,
            d: &Csr,
            unseen: &FxHashSet<u32>,
            memo: &mut [u8],
            done: &mut [bool],
            x: u32,
        ) -> bool {
            let s = l.sym;
            let xi = x as usize;
            if done[xi] {
                return memo[xi] == 2;
            }
            if memo[xi] == 1 {
                return false; // a cycle is not proof of emptiness
            }
            memo[xi] = 1;
            let ok = matches!(s.kind[xi], KIND_CLASS | KIND_INTERFACE)
                && s.flags[xi] & FLAG_SEALED == 0
                && !unseen.contains(&x)
                && !l
                    .kids(x)
                    .iter()
                    .any(|&k| is_inheritable_kind(s.kind[k as usize]))
                && d.get(xi)
                    .iter()
                    .filter(|&&t| is_type_kind(s.kind[t as usize]))
                    .all(|&t| is_marker(l, d, unseen, memo, done, t));
            memo[xi] = if ok { 2 } else { 1 };
            done[xi] = true;
            ok
        }
        (0..n)
            .map(|i| {
                matches!(s.kind[i], KIND_CLASS | KIND_INTERFACE)
                    && is_marker(self, declares, unseen, &mut memo, &mut done, i as u32)
            })
            .collect()
    }

    /// Classes whose supertype chain carries type arguments, and those arguments per class.
    fn targ_prep(&self, declares: &Csr, type_args: &Csr) -> TargPrep {
        let s = self.sym;
        let n = s.nodes();
        let mut prep = TargPrep::default();
        if type_args.val.is_empty() {
            return prep;
        }
        let supers = |x: u32| -> Vec<u32> {
            declares
                .get(x as usize)
                .iter()
                .copied()
                .filter(|&t| is_type_kind(s.kind[t as usize]))
                .collect()
        };
        fn args_through(
            x: u32,
            stack: &mut Vec<u32>,
            through: &mut FxHashMap<u32, Vec<u32>>,
            type_args: &Csr,
            supers: &dyn Fn(u32) -> Vec<u32>,
        ) -> Vec<u32> {
            if let Some(v) = through.get(&x) {
                return v.clone();
            }
            let mut out: Vec<u32> = type_args.get(x as usize).to_vec();
            stack.push(x);
            for sup in supers(x) {
                if !stack.contains(&sup) {
                    out.extend(args_through(sup, stack, through, type_args, supers));
                }
            }
            stack.pop();
            out.sort_unstable();
            out.dedup();
            through.insert(x, out.clone());
            out
        }
        // Every node that declares a supertype or a type argument, in node order.
        for c in 0..n as u32 {
            if declares.get(c as usize).is_empty() && type_args.get(c as usize).is_empty() {
                continue;
            }
            if !args_through(c, &mut Vec::new(), &mut prep.through, type_args, &supers).is_empty() {
                prep.users.insert(c);
            }
        }
        prep.user_names = prep.users.iter().map(|&u| s.name[u as usize]).collect();
        prep
    }

    /// Members reachable from `t` and its supertypes, by name.
    fn layer_of(&self, t: u32, declares: &Csr) -> FxHashMap<u32, Vec<u32>> {
        let s = self.sym;
        let mut out: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        let mut seen: FxHashSet<u32> = FxHashSet::from_iter([t]);
        let mut queue = vec![t];
        while let Some(c) = queue.pop() {
            for &k in self.kids(c) {
                if is_inheritable_kind(s.kind[k as usize]) && self.reachable[k as usize] {
                    out.entry(s.name[k as usize]).or_default().push(k);
                }
            }
            for &sup in declares.get(c as usize) {
                if is_type_kind(s.kind[sup as usize]) && seen.insert(sup) {
                    queue.push(sup);
                }
            }
        }
        out
    }

    /// A file that names a class inheriting `Base<T>` resolves its tokens against T's members:
    /// (the type arguments it resolved through, (source, targets) per declaration that hit one).
    fn targ_file(
        &self,
        f: usize,
        refs: &FileRefs,
        prep: &Prep,
    ) -> Option<(Vec<u32>, Vec<(u32, Vec<u32>)>)> {
        let s = self.sym;
        let tp = &prep.targ;
        if tp.users.is_empty() {
            return None;
        }
        let range = s.file_nodes(f);
        if range.is_empty() {
            return None;
        }
        let base = range.start;
        let mut file_tokens: FxHashSet<u32> = FxHashSet::default();
        for r in &refs.refs {
            file_tokens.extend(r.iter().copied());
        }
        let local_users: Vec<u32> = range.clone().filter(|x| tp.users.contains(x)).collect();
        let hit_names: Vec<u32> = file_tokens
            .iter()
            .copied()
            .filter(|w| tp.user_names.contains(w))
            .collect();
        if local_users.is_empty() && hit_names.is_empty() {
            return None;
        }
        let scope = self.scope(f, refs);
        let mut hit_users: FxHashSet<u32> = local_users.into_iter().collect();
        for tok in hit_names {
            hit_users.extend(
                scope
                    .resolve(tok)
                    .into_iter()
                    .filter(|x| tp.users.contains(x)),
            );
        }
        let mut args: Vec<u32> = hit_users
            .iter()
            .flat_map(|&u| tp.through.get(&u).cloned().unwrap_or_default())
            .collect();
        args.sort_unstable();
        args.dedup();
        if args.is_empty() {
            return None;
        }
        let layers: Vec<FxHashMap<u32, Vec<u32>>> = args
            .iter()
            .map(|&t| self.layer_of(t, &prep.declares))
            .collect();
        let mut links = Vec::new();
        for node in range {
            let toks = refs
                .refs
                .get((node - base) as usize)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let hits: Vec<u32> = toks
                .iter()
                .flat_map(|tok| {
                    layers
                        .iter()
                        .flat_map(move |d| d.get(tok).into_iter().flatten().copied())
                })
                .collect();
            if !hits.is_empty() {
                links.push((node, scope.keep(hits)));
            }
        }
        Some((args, links))
    }
}

impl<'l, 'a> Scope<'l, 'a> {
    fn keep(&self, mut nodes: Vec<u32>) -> Vec<u32> {
        let l = self.l;
        let s = l.sym;
        let home = s.file_module[self.file];
        nodes.retain(|&n| {
            let rf = s.file[l.root[n as usize] as usize] as usize;
            (self.from_test || !s.file_test[rf]) && l.vis.sees(home, s.file_module[rf])
        });
        nodes
    }

    fn direct(&self, name: u32, out: &mut Vec<u32>) {
        if let Some(v) = self.local.get(&name) {
            out.extend(v);
        }
        if let Some(v) = self.imported.get(&name) {
            out.extend(v);
        }
        for &pkg in &self.probed {
            if let Some(v) = self.l.pkg_index.get(&(pkg, name)) {
                out.extend(v);
            }
        }
    }

    fn resolve(&self, name: u32) -> Vec<u32> {
        let mut hits = Vec::new();
        self.direct(name, &mut hits);
        if hits.is_empty() {
            hits
        } else {
            self.keep(hits)
        }
    }

    /// One declaration's tokens: (written, implicit-receiver only).
    fn resolve_split(&self, names: &[u32], inherited: &[u32]) -> (FxHashSet<u32>, FxHashSet<u32>) {
        let mut direct: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for &name in names {
            let mut hits = Vec::new();
            self.direct(name, &mut hits);
            if !hits.is_empty() {
                direct.insert(name, hits);
            }
        }
        let mut members: Vec<u32> = Vec::new();
        for hits in direct.values() {
            for &n in hits {
                if self.l.under_names[n as usize] > 1 {
                    members.push(n);
                }
            }
        }
        let mut near: FxHashSet<u32> = FxHashSet::default();
        let mut far: FxHashSet<u32> = FxHashSet::default();
        for &name in names {
            let mut hits: Vec<u32> = direct.get(&name).cloned().unwrap_or_default();
            for &m in &members {
                hits.extend(self.l.under_named(m, name));
            }
            if !hits.is_empty() {
                near.extend(self.keep(hits));
            }
            let implicit: Vec<u32> = inherited
                .iter()
                .flat_map(|&d| self.l.under_named(d, name))
                .collect();
            if !implicit.is_empty() {
                far.extend(self.keep(implicit));
            }
        }
        far.retain(|x| !near.contains(x));
        (near, far)
    }

    fn resolve_type_path(&self, path: &[u32]) -> Vec<u32> {
        let s = self.l.sym;
        let mut hits: Vec<u32> = self
            .resolve(path[0])
            .into_iter()
            .filter(|&n| is_type_kind(s.kind[n as usize]))
            .collect();
        for &seg in &path[1..] {
            hits = hits
                .iter()
                .flat_map(|&n| self.l.kids(n).iter().copied())
                .filter(|&k| s.name[k as usize] == seg)
                .collect();
        }
        let mut v: Vec<u32> = hits
            .into_iter()
            .filter(|&n| is_type_kind(s.kind[n as usize]))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    fn resolve_supertype(&self, name: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .resolve(name)
            .into_iter()
            .flat_map(|n| self.l.inherited(n))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// File-level widening for an import no declaration body accounts for.
    fn unattributed_import(
        &self,
        imp: &Import,
        file_tokens: &FxHashSet<u32>,
        in_code: &FxHashSet<u32>,
    ) -> Vec<u32> {
        let l = self.l;
        if imp.star {
            let Some(&pkg) = imp.prefixes.last() else {
                return vec![];
            };
            let Some(roots) = l.pkg_roots.get(&pkg) else {
                return vec![];
            };
            if roots
                .iter()
                .any(|&r| file_tokens.contains(&l.sym.name[r as usize]))
            {
                return vec![];
            }
            return self.keep(
                roots
                    .iter()
                    .copied()
                    .filter(|&r| l.operator_ids.contains(&l.sym.name[r as usize]))
                    .collect(),
            );
        }
        let simple = if imp.alias != NONE {
            imp.alias
        } else {
            *imp.segs.last().unwrap_or(&NONE)
        };
        if file_tokens.contains(&simple) {
            return vec![];
        }
        if !in_code.contains(&simple) && !l.operator_ids.contains(&simple) {
            return vec![];
        }
        let mut out = Vec::new();
        for n in l.resolve_fqn(imp) {
            l.descendants(n, &mut out);
        }
        out.sort_unstable();
        out.dedup();
        self.keep(out)
    }
}

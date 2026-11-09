//! `kt-affected unreferenced` and `kt-affected test-reach`: what the declaration graph says about
//! code nothing uses, and code no test runs.
//!
//! Both read the same resolved graph `select` does. Each errs one way only:
//!
//! - `unreferenced` reports a declaration only when no edge reaches it, its name is written by no
//!   other code that can see it (calls through a receiver whose type the linker infers get no
//!   edge), and no rule says something outside the sources calls it. A miss keeps code; it never
//!   reports code that is used.
//! - `test-reach` walks forward from tests over every edge, every override, every DI-bound
//!   implementation, every member whose name the code writes and every container a member lives
//!   in: a prod declaration it does not reach is certainly run by no test, and a test that reaches
//!   no prod declaration certainly tests none.

use crate::cache::Packed;
use crate::config::Config;
use crate::discover::{Build, is_test_source_set};
use crate::git::Repo;
use crate::parse::{
    FLAG_ANY_SEMANTICS, FLAG_CTOR_PROP, FLAG_INITIALIZED, FLAG_OVERRIDE, FLAG_PRIVATE,
    FLAG_TEST_ROOT, KIND_CLASS, KIND_CONSTRUCTOR, KIND_ENUM_ENTRY, KIND_FUN, KIND_INIT,
    KIND_INTERFACE, KIND_OBJECT, KIND_TYPEALIAS, KIND_VAL, KIND_VAR, is_type_kind,
};
use crate::route::Modules;
use crate::seeds::{self, View};
use crate::{CoverageCount, DeclCoverage, Declaration, Inbound, TestReach, TestUnit, Unreferenced};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The version of the JSON both commands print.
pub const SCHEMA: u32 = 1;

const NONE: u32 = u32::MAX;

/// Names Kotlin calls without their being written (`a + b`, `x[i]`, `by lazy`, destructuring,
/// string templates, `==`).
const IMPLICIT_NAMES: &[&str] = &[
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
    "toString",
    "hashCode",
];

/// Words a referrer of an enum writes when it uses every entry without naming one.
const ENUM_WIDE: &[&str] = &[
    "entries",
    "values",
    "valueOf",
    "enumValues",
    "enumValueOf",
    "enumEntries",
];

/// Annotations marking a test method: a unit `test-reach` judges on its own.
const TEST_ANNOTATIONS: &[&str] = &[
    "Test",
    "ParameterizedTest",
    "RepeatedTest",
    "TestFactory",
    "TestTemplate",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Prod,
    Test,
    Fixture,
}

impl Source {
    fn idx(self) -> usize {
        self as usize
    }
    fn name(self) -> &'static str {
        match self {
            Source::Prod => "prod",
            Source::Test => "test",
            Source::Fixture => "fixture",
        }
    }
}

/// The source set kind of a path: `testFixtures*` is a fixture, a test source set a test,
/// anything else (no source set included) prod.
/// Also whether the file lies in a source set at all: one in no source set (a reference copy
/// beside the sources) is compiled by nothing, so nothing in it is reported.
fn source_of(tmods: &Modules, path: &str) -> (Source, bool) {
    let none = BTreeMap::new();
    let set = match tmods.source_set_of(path) {
        Some((_, s)) => Some(s),
        None => crate::discover::source_set_at(&none, path),
    };
    let kind = match set {
        Some(s) if s.starts_with("testFixtures") => Source::Fixture,
        Some(s) if is_test_source_set(s) => Source::Test,
        _ => Source::Prod,
    };
    (kind, set.is_some())
}

/// Class names spelled out where a framework loads them by name: string literals in build
/// scripts (`implementationClass = "a.b.Plugin"`), `META-INF/services` files and Android
/// manifests. Returns (fully qualified names, `.Relative` manifest names).
fn named_by_config(root: &Path, build: &Build) -> (BTreeSet<String>, Vec<String>) {
    let mut scripts: Vec<String> = build.inputs.clone();
    let mut dirs: Vec<String> = Vec::new();
    for p in build
        .projects
        .iter()
        .chain(build.included.iter().flat_map(|i| &i.projects))
    {
        scripts.extend(p.build_file.iter().cloned());
        dirs.push(p.dir.clone());
    }
    let mut fq: BTreeSet<String> = BTreeSet::new();
    let mut rel: Vec<String> = Vec::new();
    let qualified = |s: &str| {
        s.contains('.')
            && s.split('.').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
            })
    };
    for s in &scripts {
        if !(s.ends_with(".kts") || s.ends_with(".gradle")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(s)) else {
            continue;
        };
        // Dotted names in code (`a.b.Stanza.inject(..)`, imports) and strings alike.
        for chain in text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '$'))) {
            let chain = chain.trim_matches('.');
            if qualified(chain) {
                fq.insert(chain.replace('$', "."));
            }
        }
    }
    for d in dirs {
        let Ok(sets) = std::fs::read_dir(root.join(&d).join("src")) else {
            continue;
        };
        for set in sets.flatten() {
            let base = set.path();
            if let Ok(text) = std::fs::read_to_string(base.join("AndroidManifest.xml")) {
                for part in text.split("android:name=\"").skip(1) {
                    let name = part.split('"').next().unwrap_or("");
                    if name.starts_with('.') {
                        rel.push(name.to_string());
                    } else if qualified(name) {
                        fq.insert(name.to_string());
                    }
                }
            }
            if let Ok(svc) = std::fs::read_dir(base.join("resources/META-INF/services")) {
                for f in svc.flatten() {
                    let text = std::fs::read_to_string(f.path()).unwrap_or_default();
                    for line in text.lines() {
                        let line = line.split('#').next().unwrap_or("").trim();
                        if qualified(line) {
                            fq.insert(line.replace('$', "."));
                        }
                    }
                }
            }
        }
    }
    (fq, rel)
}

/// The graph read for analysis: per-node tables, per-file source kind and module, per-declaration
/// refs.
struct Code<'a> {
    v: View<'a>,
    root: &'a Path,
    source: Vec<Source>,
    /// Per file: it lies in a source set.
    compiled: Vec<bool>,
    /// Per file: index of its root-build module, or NONE.
    module: Vec<u32>,
    /// Per module index: the module indices it may reference (itself included).
    sees: Vec<FxHashSet<u32>>,
    refs: Vec<crate::cache::DeclRefs>,
    texts: std::cell::RefCell<FxHashMap<u32, Option<std::rc::Rc<Vec<String>>>>>,
}

impl<'a> Code<'a> {
    fn new(
        root: &'a Path,
        build: &Build,
        cfg: &Config,
        p: &'a Packed,
        dir: &Path,
    ) -> Result<Code<'a>, String> {
        let v = View::new(p);
        let (mods, _) = crate::select::modules_of(root, build, cfg);
        let tmods = crate::select::with_included(&mods, build);
        let ids: Vec<String> = mods.list.iter().map(|m| m.id.clone()).collect();
        let index: FxHashMap<&str, u32> = ids
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i as u32))
            .collect();
        let sees = ids
            .iter()
            .map(|id| {
                mods.closure(&[id.clone()].into_iter().collect())
                    .iter()
                    .filter_map(|m| index.get(m.as_str()).copied())
                    .collect()
            })
            .collect();
        let (mut source, mut compiled, mut module) = (Vec::new(), Vec::new(), Vec::new());
        for f in 0..p.n_files() {
            let path = p.file_path(f);
            let (s, c) = source_of(&tmods, path);
            source.push(s);
            compiled.push(c);
            module.push(
                mods.module_of(path)
                    .and_then(|m| index.get(m.id.as_str()).copied())
                    .unwrap_or(NONE),
            );
        }
        let refs = crate::cache::decl_refs(dir, p)
            .ok_or_else(|| "the graph's parses.bin is missing or stale".to_string())?;
        Ok(Code {
            v,
            root,
            source,
            compiled,
            module,
            sees,
            refs,
            texts: Default::default(),
        })
    }

    fn src(&self, n: u32) -> Source {
        self.source[self.v.file[n as usize] as usize]
    }

    /// Can code in file `from` reference a declaration in file `to`?
    fn sees_file(&self, from: u32, to: u32) -> bool {
        let (a, b) = (self.module[from as usize], self.module[to as usize]);
        a == NONE || b == NONE || a == b || self.sees[a as usize].contains(&b)
    }

    fn refs_of(&self, n: u32) -> &[u32] {
        let f = self.v.file[n as usize];
        let local = (n - self.v.p.file_base(f as usize)) as usize;
        self.refs[f as usize]
            .refs
            .get(local)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Annotations of `n` (simple names), an import alias (`import kotlin.test.Test as KTest`)
    /// read as the name it imports.
    fn annotations(&self, n: u32) -> Vec<u32> {
        let aliases = &self.refs[self.v.file[n as usize] as usize].aliases;
        self.v
            .p
            .node_annotations(n as usize)
            .iter()
            .map(|&a| {
                aliases
                    .iter()
                    .find(|(al, _)| *al == a)
                    .map(|&(_, real)| real)
                    .unwrap_or(a)
            })
            .collect()
    }

    /// Does `r`'s own text write `T::class` for the type `t` (reflection over its members or
    /// entries, `@EnumSource(T::class)`)?
    fn writes_class_literal(&self, r: u32, t: u32) -> bool {
        let v = &self.v;
        let Some(lines) = self.lines(v.file[r as usize]) else {
            return true;
        };
        let name = v.p.str(v.name[t as usize]);
        let (lo, hi) = (v.lo[r as usize] as usize, v.hi[r as usize] as usize);
        lines.iter().take(hi).skip(lo.saturating_sub(1)).any(|l| {
            l.match_indices(name).any(|(at, _)| {
                l[at + name.len()..].trim_start().starts_with("::class")
                    && (at == 0 || !l.as_bytes()[at - 1].is_ascii_alphanumeric())
            })
        })
    }

    /// The file writes `"name"` (or `"Type#name"`) in a string: reflection or a test-source
    /// annotation (`@MethodSource("name")`) may look the declaration up by name.
    fn named_in_string(&self, n: u32) -> bool {
        let name = self.v.p.str(self.v.name[n as usize]);
        let Some(lines) = self.lines(self.v.file[n as usize]) else {
            return true;
        };
        let (quoted, hashed) = (format!("\"{name}\""), format!("#{name}\""));
        lines
            .iter()
            .any(|l| l.contains(&quoted) || l.contains(&hashed))
    }

    /// `r` is `n` or declared inside it.
    fn within(&self, r: u32, n: u32) -> bool {
        if self.v.file[r as usize] != self.v.file[n as usize] || r < n {
            return false;
        }
        let mut x = r as i32;
        while x >= 0 {
            if x as u32 == n {
                return true;
            }
            x = self.v.parent[x as usize];
        }
        false
    }

    fn label(&self, n: u32) -> String {
        let v = &self.v;
        let mut parts = Vec::new();
        let mut m = n as i32;
        while m >= 0 {
            parts.push(v.p.str(v.name[m as usize]));
            m = v.parent[m as usize];
        }
        parts.reverse();
        parts.join(".")
    }

    fn fq(&self, n: u32) -> String {
        let pkg = self.v.p.file_package(self.v.file[n as usize] as usize);
        if pkg.is_empty() {
            self.label(n)
        } else {
            format!("{pkg}.{}", self.label(n))
        }
    }

    fn lines(&self, f: u32) -> Option<std::rc::Rc<Vec<String>>> {
        self.texts
            .borrow_mut()
            .entry(f)
            .or_insert_with(|| {
                std::fs::read(self.root.join(self.v.p.file_path(f as usize)))
                    .ok()
                    .map(|b| {
                        std::rc::Rc::new(
                            String::from_utf8_lossy(&b)
                                .split('\n')
                                .map(str::to_string)
                                .collect(),
                        )
                    })
            })
            .clone()
    }

    /// The text before a declaration's name: its annotations and modifiers.
    fn header(&self, n: u32) -> String {
        let v = &self.v;
        let i = n as usize;
        let Some(lines) = self.lines(v.file[i]) else {
            return String::new();
        };
        let (lo, hi) = (
            v.lo[i] as usize,
            (v.hi[i] as usize).min(v.lo[i] as usize + 8),
        );
        let text = lines
            .iter()
            .take(hi)
            .skip(lo.saturating_sub(1))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let name = v.p.str(v.name[i]);
        let at = word_at(&text, name).unwrap_or(text.len());
        let mut head = strip_annotations(&text[..at]);
        // A constructor property shares its line with its neighbours.
        if let Some(cut) = head.rfind(['(', ',', '{', ';', '}']) {
            head = head[cut + 1..].to_string();
        }
        head
    }

    fn kind_name(&self, n: u32) -> &'static str {
        let i = n as usize;
        match self.v.kind[i] {
            KIND_CLASS => "class",
            KIND_INTERFACE => "interface",
            KIND_OBJECT => "object",
            KIND_TYPEALIAS => "typealias",
            KIND_ENUM_ENTRY => "enum_entry",
            KIND_FUN => "function",
            KIND_VAL | KIND_VAR if self.v.dflags[i] & FLAG_CTOR_PROP != 0 => "constructor_property",
            KIND_VAL | KIND_VAR => "property",
            KIND_INIT => "init",
            KIND_CONSTRUCTOR => "constructor",
            _ => "declaration",
        }
    }

    /// Prod declarations with an edge to `n` from outside it (at most 50), in node order.
    fn prod_referrers(&self, n: u32) -> Vec<crate::Referrer> {
        let v = &self.v;
        let mut rs: Vec<u32> = v
            .refby(n)
            .iter()
            .copied()
            .filter(|&r| !self.within(r, n) && self.src(r) == Source::Prod)
            .collect();
        rs.sort_unstable();
        rs.dedup();
        rs.into_iter()
            .take(50)
            .map(|r| crate::Referrer {
                fq_name: self.fq(r),
                file: v.p.file_path(v.file[r as usize] as usize).to_string(),
                line: v.lo[r as usize],
            })
            .collect()
    }

    fn record(&self, n: u32, inbound: [u32; 3], named: [bool; 3]) -> Declaration {
        let v = &self.v;
        let i = n as usize;
        let head = self.header(n);
        let words: BTreeSet<&str> = head
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| !w.is_empty())
            .collect();
        let visibility = if v.dflags[i] & FLAG_PRIVATE != 0 || words.contains("private") {
            "private"
        } else if words.contains("protected") {
            "protected"
        } else if words.contains("internal") {
            "internal"
        } else {
            "public"
        };
        Declaration {
            fq_name: self.fq(n),
            kind: self.kind_name(n).into(),
            file: v.p.file_path(v.file[i] as usize).to_string(),
            start_line: v.lo[i],
            end_line: v.hi[i],
            visibility: visibility.into(),
            source: self.src(n).name().into(),
            member: v.parent[i] >= 0,
            operator: words.contains("operator")
                || (v.kind[i] == KIND_FUN
                    && IMPLICIT_NAMES.contains(&v.p.str(v.name[i]))
                    && words.contains("override")),
            inbound: Inbound {
                prod: inbound[0],
                test: inbound[1],
                fixture: inbound[2],
            },
            named_in: [Source::Prod, Source::Test, Source::Fixture]
                .into_iter()
                .filter(|s| named[s.idx()])
                .map(|s| s.name().to_string())
                .collect(),
            change: None,
            referrers: self.prod_referrers(n),
        }
    }
}

/// Byte offset of `word` as a whole identifier in `text`.
fn word_at(text: &str, word: &str) -> Option<usize> {
    let b = text.as_bytes();
    let is_w = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'`';
    text.match_indices(word).map(|(i, _)| i).find(|&i| {
        (i == 0 || !is_w(b[i - 1])) && (i + word.len() >= b.len() || !is_w(b[i + word.len()]))
    })
}

/// `text` with `@Ann`, `@a.b.Ann(...)` and `@get:Ann` removed.
fn strip_annotations(text: &str) -> String {
    let mut out = String::new();
    let c: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '@' {
            i += 1;
            while i < c.len() && (c[i].is_alphanumeric() || matches!(c[i], '_' | '.' | ':')) {
                i += 1;
            }
            if i < c.len() && c[i] == '(' {
                let mut depth = 0;
                while i < c.len() {
                    match c[i] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// A `*` glob over the whole text.
fn glob(pat: &str, text: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == text;
    }
    let mut rest = text;
    for (k, part) in parts.iter().enumerate() {
        if k == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if k == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
    }
    true
}

/// Facts both commands derive from the graph once.
struct Facts {
    /// member -> the members that override it.
    overriders: FxHashMap<u32, Vec<u32>>,
    /// Classes a DI framework or a typed dispatch base instantiates.
    bound: FxHashSet<u32>,
    /// type -> its subtypes in `bound`.
    bound_subtypes: FxHashMap<u32, Vec<u32>>,
}

fn facts(c: &Code, cfg: &Config) -> Facts {
    let v = &c.v;
    let mut overriders: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for o in 0..v.n as u32 {
        let i = o as usize;
        if v.dflags[i] & FLAG_OVERRIDE == 0 || v.parent[i] < 0 {
            continue;
        }
        for &s in v.declares(v.parent[i] as u32) {
            if s != o && v.name[s as usize] == v.name[i] && v.parent[s as usize] >= 0 {
                overriders.entry(s).or_default().push(o);
            }
        }
    }
    let mut bound = seeds::di_bound(v, &cfg.di.binding_annotations, &cfg.di.provider_annotations);
    let base_names: FxHashSet<u32> = cfg
        .dispatch
        .iter()
        .flat_map(|d| d.bases.iter())
        .filter_map(|b| v.string_id(b))
        .collect();
    if !base_names.is_empty() {
        for t in 0..v.n as u32 {
            if v.parent[t as usize] < 0
                && v.kind[t as usize] == KIND_CLASS
                && v.declares(t)
                    .iter()
                    .any(|&s| v.parent[s as usize] < 0 && base_names.contains(&v.name[s as usize]))
            {
                bound.insert(t);
            }
        }
    }
    let mut bound_subtypes: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for &b in &bound {
        for &s in v.declares(b) {
            if is_type_kind(v.kind[s as usize]) {
                bound_subtypes.entry(s).or_default().push(b);
            }
        }
    }
    Facts {
        overriders,
        bound,
        bound_subtypes,
    }
}

/// Kinds a report may list: not init blocks, secondary constructors or unnamed companions.
fn reportable(c: &Code, n: u32) -> bool {
    let i = n as usize;
    let v = &c.v;
    c.compiled[v.file[i] as usize]
        && !matches!(v.kind[i], KIND_INIT | KIND_CONSTRUCTOR)
        && !(v.kind[i] == KIND_OBJECT && v.p.str(v.name[i]) == "Companion")
}

/// Callables, reached through a receiver the linker may not type: gated on their name.
fn gated(c: &Code, n: u32) -> bool {
    matches!(
        c.v.kind[n as usize],
        KIND_FUN | KIND_VAL | KIND_VAR | KIND_ENUM_ENTRY
    )
}

/// Edge referrers of `n` from outside it, by source kind.
fn inbound(c: &Code, n: u32, into: &mut [u32; 3]) {
    for &r in c.v.refby(n) {
        if !c.within(r, n) {
            into[c.src(r).idx()] += 1;
        }
    }
}

pub struct Inputs<'a> {
    pub root: &'a Path,
    pub build: &'a Build,
    pub packed: &'a Packed,
    pub cfg: &'a Config,
    pub dir: &'a Path,
    pub repo: Option<&'a Repo>,
    /// Report prod declarations with at most this many prod referrers (0: unreferenced).
    pub max_inbound: u32,
}

// ------------------------------------------------------------------ unreferenced

pub fn unreferenced(inp: &Inputs) -> Result<Unreferenced, String> {
    let c = Code::new(inp.root, inp.build, inp.cfg, inp.packed, inp.dir)?;
    let v = &c.v;
    let cfg = &inp.cfg.unreferenced;
    let facts = facts(&c, inp.cfg);
    let ids = |names: &[String]| -> FxHashSet<u32> {
        names.iter().filter_map(|a| v.string_id(a)).collect()
    };
    let entry_ann = ids(&cfg.entry_annotations);
    let entry_names = ids(&cfg.entry_names);
    let entry_supers: FxHashSet<&str> = cfg.entry_supertypes.iter().map(String::as_str).collect();
    let enum_wide = ids(&ENUM_WIDE.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let unparsed: Vec<u32> = (0..v.p.n_files() as u32)
        .filter(|&f| v.p.file_error(f as usize))
        .collect();

    let (by_config, by_manifest) = named_by_config(inp.root, inp.build);
    let test_ann = ids(&TEST_ANNOTATIONS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>());
    // Types holding tests: a subclass runs them.
    let mut test_bases: FxHashSet<u32> = v.test_class.keys().copied().collect();
    for n in 0..v.n as u32 {
        let i = n as usize;
        if v.parent[i] >= 0
            && v.kind[i] == KIND_FUN
            && c.src(n) != Source::Prod
            && c.annotations(n).iter().any(|a| test_ann.contains(a))
        {
            test_bases.insert(v.parent[i] as u32);
        }
    }
    let simple = |s: u32| -> &str {
        let w = v.p.str(s);
        w.rsplit('.').next().unwrap_or(w)
    };
    let has_entry_super = |t: u32| -> bool {
        !entry_supers.is_empty()
            && std::iter::once(t)
                .chain(v.declares(t).iter().copied())
                .any(|x| {
                    v.p.node_supers(x as usize)
                        .iter()
                        .any(|&s| entry_supers.contains(simple(s)))
                })
    };
    let entry = |n: u32| -> bool {
        let i = n as usize;
        if v.dflags[i] & (FLAG_OVERRIDE | FLAG_TEST_ROOT) != 0
            || facts.overriders.contains_key(&n)
            || entry_names.contains(&v.name[i])
            || c.annotations(n).iter().any(|a| entry_ann.contains(a))
        {
            return true;
        }
        if !cfg.ignored_names.is_empty() {
            let name = v.p.str(v.name[i]);
            let fq = c.fq(n);
            if cfg
                .ignored_names
                .iter()
                .any(|g| glob(g, name) || glob(g, &fq))
            {
                return true;
            }
        }
        if is_type_kind(v.kind[i]) && has_entry_super(n) {
            return true;
        }
        // Named by a build script or framework config (a member through `a.b.T.member`, a type
        // through any name under it).
        if !by_config.is_empty() {
            let fq = c.fq(n);
            if by_config.contains(&fq)
                || (is_type_kind(v.kind[i])
                    && by_config
                        .range(format!("{fq}.")..)
                        .next()
                        .is_some_and(|s| s.starts_with(&format!("{fq}."))))
            {
                return true;
            }
        }
        if is_type_kind(v.kind[i]) {
            let fq = c.fq(n);
            if by_manifest.iter().any(|r| fq.ends_with(r.as_str())) {
                return true;
            }
            // A test class whose tests it inherits: the runner instantiates it.
            if c.src(n) != Source::Prod && v.declares(n).iter().any(|s| test_bases.contains(s)) {
                return true;
            }
        }
        if v.dflags[i] & FLAG_CTOR_PROP != 0 && v.parent[i] >= 0 {
            let p = v.parent[i] as usize;
            // Read by equals/copy/destructuring/serialization, or by whatever instantiates an
            // entry type, without being named.
            if v.dflags[p] & FLAG_ANY_SEMANTICS != 0
                || c.annotations(p as u32)
                    .iter()
                    .any(|a| entry_ann.contains(a))
            {
                return true;
            }
        }
        false
    };

    // Pass 1: edges, entries, DI.
    let mut counts: FxHashMap<u32, [u32; 3]> = FxHashMap::default();
    let mut cands: Vec<u32> = Vec::new();
    for n in 0..v.n as u32 {
        if !reportable(&c, n) || entry(n) {
            continue;
        }
        let mut k = [0u32; 3];
        inbound(&c, n, &mut k);
        if facts.bound.contains(&n) {
            let supers: Vec<u32> = v
                .declares(n)
                .iter()
                .copied()
                .filter(|&s| is_type_kind(v.kind[s as usize]))
                .collect();
            if supers.is_empty() && !v.p.node_supers(n as usize).is_empty() {
                continue; // bound to a type outside the sources: the framework hands it out
            }
            for s in supers {
                for &r in v.refby(s) {
                    if !c.within(r, n) {
                        k[c.src(r).idx()] += 1;
                    }
                }
            }
        }
        let open = match c.src(n) {
            Source::Prod => k[0] <= inp.max_inbound,
            _ => k.iter().sum::<u32>() <= inp.max_inbound,
        };
        if open {
            counts.insert(n, k);
            cands.push(n);
        }
    }

    // Pass 2: the name gate, for callables.
    let want: FxHashSet<u32> = cands
        .iter()
        .filter(|&&n| gated(&c, n))
        .map(|&n| v.name[n as usize])
        .collect();
    let mut users: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for d in 0..v.n as u32 {
        for &w in c.refs_of(d) {
            if want.contains(&w) {
                users.entry(w).or_default().push(d);
            }
        }
    }
    let mut by_name: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for d in 0..v.n as u32 {
        if want.contains(&v.name[d as usize]) {
            by_name.entry(v.name[d as usize]).or_default().push(d);
        }
    }
    let mut named: FxHashMap<u32, [bool; 3]> = FxHashMap::default();
    cands.retain(|&n| {
        if !gated(&c, n) {
            return true;
        }
        let i = n as usize;
        let private = v.dflags[i] & FLAG_PRIVATE != 0;
        let (f, w) = (v.file[i], v.name[i]);
        let mut seen = [false; 3];
        for &d in users.get(&w).into_iter().flatten() {
            let fd = v.file[d as usize];
            if c.within(d, n)
                || v.refby(n).contains(&d)
                || (private && fd != f)
                || !c.sees_file(fd, f)
            {
                continue;
            }
            seen[c.src(d).idx()] = true;
        }
        // A declaration's refs leave out its own name: `fun save() = store.save()` writes `save`
        // without a ref. A same-named declaration whose text writes the name again may call it.
        let prod_only = c.src(n) == Source::Prod;
        let done = |s: &[bool; 3]| {
            if prod_only {
                s[0]
            } else {
                s.iter().any(|&b| b)
            }
        };
        if !done(&seen) {
            let name = v.p.str(w);
            for &d in by_name.get(&w).into_iter().flatten() {
                let fd = v.file[d as usize];
                if d == n
                    || c.within(d, n)
                    || v.refby(n).contains(&d)
                    || (private && fd != f)
                    || !c.sees_file(fd, f)
                {
                    continue;
                }
                let Some(lines) = c.lines(fd) else {
                    seen[c.src(d).idx()] = true; // unreadable: assume it calls it
                    continue;
                };
                let (lo, hi) = (v.lo[d as usize] as usize, v.hi[d as usize] as usize);
                let hits: usize = lines
                    .iter()
                    .take(hi)
                    .skip(lo.saturating_sub(1))
                    .map(|l| count_words(l, name))
                    .sum();
                if hits > 1 {
                    seen[c.src(d).idx()] = true;
                }
                if done(&seen) {
                    break;
                }
            }
        }
        named.insert(n, seen);
        !done(&seen)
    });

    // Pass 3: enum-wide use, files that do not parse, `actual`, overload groups.
    let mut enum_used: FxHashMap<u32, bool> = FxHashMap::default();
    let errored_sees =
        |n: u32| -> bool { unparsed.iter().any(|&f| c.sees_file(f, v.file[n as usize])) };
    cands.retain(|&n| {
        let i = n as usize;
        if v.kind[i] == KIND_ENUM_ENTRY && v.parent[i] >= 0 {
            let e = v.parent[i] as u32;
            let used = *enum_used.entry(e).or_insert_with(|| {
                c.annotations(e).iter().any(|a| entry_ann.contains(a))
                    || v.refby(e).iter().any(|&r| {
                        c.refs_of(r).iter().any(|w| enum_wide.contains(w))
                            || c.writes_class_literal(r, e)
                    })
            });
            if used {
                return false;
            }
        }
        if !unparsed.is_empty() && errored_sees(n) {
            return false;
        }
        if c.named_in_string(n) {
            return false;
        }
        let head = c.header(n);
        // A companion is what the class name means as a value (`ctx[Key]`, `Base(Key)`): its
        // members are reported, never it.
        !(word_at(&head, "actual").is_some()
            || word_at(&head, "external").is_some()
            || (v.kind[n as usize] == KIND_OBJECT && word_at(&head, "companion").is_some()))
    });
    let open: FxHashSet<u32> = cands.iter().copied().collect();
    let group = |n: u32| -> (i64, u32) {
        let i = n as usize;
        // Overloads share a name in one scope: a class, or a package for top-level ones.
        let scope = if v.parent[i] >= 0 {
            v.parent[i] as i64
        } else {
            -1 - v
                .string_id(v.p.file_package(v.file[i] as usize))
                .map(|s| s as i64)
                .unwrap_or(-1)
        };
        (scope, v.name[i])
    };
    let mut members: FxHashMap<(i64, u32), Vec<u32>> = FxHashMap::default();
    for &n in &cands {
        members.entry(group(n)).or_default();
    }
    for n in 0..v.n as u32 {
        if let Some(g) = members.get_mut(&group(n)) {
            g.push(n);
        }
    }
    cands.retain(|&n| {
        members[&group(n)]
            .iter()
            .all(|m| open.contains(m) || !reportable(&c, *m))
    });

    let mut out: Vec<Declaration> = cands
        .iter()
        .map(|&n| c.record(n, counts[&n], named.get(&n).copied().unwrap_or_default()))
        .collect();
    if let Some(repo) = inp.repo {
        let keep = since_base(&c, repo, &cands)?;
        out = cands
            .iter()
            .zip(out)
            .filter_map(|(n, mut d)| {
                keep.get(n).map(|ch| {
                    d.change = Some(ch.to_string());
                    d
                })
            })
            .collect();
    }
    out.sort_by(|a, b| (&a.file, a.start_line).cmp(&(&b.file, b.start_line)));
    Ok(Unreferenced {
        schema: SCHEMA,
        base: inp.repo.map(|r| r.base.clone()),
        declarations: out,
        unparsed: unparsed
            .iter()
            .map(|&f| v.p.file_path(f as usize).to_string())
            .collect(),
    })
}

fn count_words(line: &str, word: &str) -> usize {
    let b = line.as_bytes();
    let is_w = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    line.match_indices(word)
        .filter(|&(i, _)| {
            (i == 0 || !is_w(b[i - 1])) && (i + word.len() >= b.len() || !is_w(b[i + word.len()]))
        })
        .count()
}

/// Of the unreferenced `cands`, those the change from `repo.base` made so ("orphaned": code at
/// the base wrote its name where it resolves, in a file the change touched) or added ("added": no
/// declaration of that name existed at the base).
fn since_base(
    c: &Code,
    repo: &Repo,
    cands: &[u32],
) -> Result<FxHashMap<u32, &'static str>, String> {
    let v = &c.v;
    let changed: Vec<String> = repo.changed()?;
    let changed_set: FxHashSet<&str> = changed.iter().map(String::as_str).collect();
    // Each changed Kotlin file at the base: (path, package, imports, declarations as (fq, refs)).
    struct Old {
        path: String,
        package: String,
        imports: Vec<(String, bool)>,
        decls: Vec<(String, Vec<String>)>,
    }
    let olds: Vec<Old> = changed
        .iter()
        .filter(|p| p.ends_with(".kt"))
        .filter_map(|p| {
            let text = repo.read_base(p)?;
            let raw = crate::parse::parse_file(&text, false);
            let mut labels: Vec<String> = Vec::with_capacity(raw.decls.len());
            for d in &raw.decls {
                let l = if d.parent >= 0 {
                    format!("{}.{}", labels[d.parent as usize], d.name)
                } else {
                    d.name.clone()
                };
                labels.push(l);
            }
            let fq = |l: &str| {
                if raw.package.is_empty() {
                    l.to_string()
                } else {
                    format!("{}.{l}", raw.package)
                }
            };
            Some(Old {
                path: p.clone(),
                package: raw.package.clone(),
                imports: raw
                    .imports
                    .iter()
                    .map(|im| (im.segs.join("."), im.star))
                    .collect(),
                decls: raw
                    .decls
                    .iter()
                    .zip(&labels)
                    .map(|(d, l)| (fq(l), d.refs.clone()))
                    .collect(),
            })
        })
        .collect();
    let existed: FxHashSet<&str> = olds
        .iter()
        .flat_map(|o| o.decls.iter().map(|(fq, _)| fq.as_str()))
        .collect();
    let path_file = &v.path_file;
    // The head declarations of the changed files, by (path, fq): what a base declaration became.
    let mut head: FxHashMap<(&str, String), u32> = FxHashMap::default();
    for o in &olds {
        if let Some(&f) = path_file.get(o.path.as_str()) {
            for n in v.file_nodes(f) {
                head.insert((o.path.as_str(), c.fq(n)), n);
            }
        }
    }
    // name -> (old file, declaration) whose base code wrote it and whose head code no longer
    // does: the references the change removed.
    let mut removed: FxHashMap<&str, Vec<(usize, usize)>> = FxHashMap::default();
    for (k, o) in olds.iter().enumerate() {
        for (j, (dfq, refs)) in o.decls.iter().enumerate() {
            let now: FxHashSet<&str> = head
                .get(&(o.path.as_str(), dfq.clone()))
                .map(|&h| c.refs_of(h).iter().map(|&w| v.p.str(w)).collect())
                .unwrap_or_default();
            for w in refs {
                if !now.contains(w.as_str()) {
                    removed.entry(w.as_str()).or_default().push((k, j));
                }
            }
        }
    }
    let mut out = FxHashMap::default();
    for &n in cands {
        let i = n as usize;
        let path = v.p.file_path(v.file[i] as usize);
        let fq = c.fq(n);
        if changed_set.contains(path) && !existed.contains(fq.as_str()) {
            out.insert(n, "added");
            continue;
        }
        let name = v.p.str(v.name[i]);
        let pkg = v.p.file_package(v.file[i] as usize);
        // The names a reference to `n` from another package would also write: its containers.
        let mut owners: Vec<&str> = Vec::new();
        let mut p = v.parent[i];
        while p >= 0 {
            owners.push(v.p.str(v.name[p as usize]));
            p = v.parent[p as usize];
        }
        let orphaned = removed.get(name).into_iter().flatten().any(|&(k, j)| {
            let o = &olds[k];
            let (dfq, refs) = &o.decls[j];
            if dfq == &fq || dfq.starts_with(&format!("{fq}.")) {
                return false; // its own code
            }
            // Visibility by the file's module now (a deleted file: anything).
            if let Some(&f) = path_file.get(o.path.as_str())
                && !c.sees_file(f, v.file[i])
            {
                return false;
            }
            let imported = o.imports.iter().any(|(seg, star)| {
                (*star && seg == pkg)
                    || *seg == format!("{pkg}.{name}")
                    || owners.last().is_some_and(|t| *seg == format!("{pkg}.{t}"))
            });
            o.path == path
                || o.package == pkg
                || imported
                || (v.parent[i] >= 0 && refs.iter().any(|w| owners.contains(&w.as_str())))
                || refs
                    .iter()
                    .any(|w| w == pkg.rsplit('.').next().unwrap_or(pkg))
        });
        if orphaned {
            out.insert(n, "orphaned");
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ test-reach

/// The forward walk: what running `start` can execute (see the module docs for the rules).
struct Walker<'a> {
    c: &'a Code<'a>,
    facts: &'a Facts,
    fwd: crate::graph::Csr,
    /// name -> callables declaring it.
    callables: FxHashMap<u32, Vec<u32>>,
    implicit: FxHashSet<u32>,
    stamp: Vec<u32>,
    generation: u32,
}

impl<'a> Walker<'a> {
    fn new(c: &'a Code<'a>, facts: &'a Facts) -> Walker<'a> {
        let v = &c.v;
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for t in 0..v.n as u32 {
            for &r in v.refby(t) {
                pairs.push((r, t));
            }
        }
        let fwd = crate::graph::Csr::from_pairs(v.n, &pairs);
        let mut callables: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for n in 0..v.n as u32 {
            if matches!(v.kind[n as usize], KIND_FUN | KIND_VAL | KIND_VAR) {
                callables.entry(v.name[n as usize]).or_default().push(n);
            }
        }
        let implicit = IMPLICIT_NAMES
            .iter()
            .filter_map(|s| v.string_id(s))
            .collect();
        Walker {
            c,
            facts,
            fwd,
            callables,
            implicit,
            stamp: vec![0; v.n],
            generation: 0,
        }
    }

    /// Walk from `start`; `stop(n)` true ends the walk early (returns true). Calls `seen` on every
    /// node reached.
    fn walk(
        &mut self,
        start: &[u32],
        mut stop: impl FnMut(u32) -> bool,
        mut seen: impl FnMut(u32),
    ) -> bool {
        let c = self.c;
        let v = &c.v;
        self.generation += 1;
        let g = self.generation;
        let mut stack: Vec<u32> = Vec::new();
        let mut names: FxHashSet<(u32, u32)> = FxHashSet::default();
        for &s in start {
            if self.stamp[s as usize] != g {
                self.stamp[s as usize] = g;
                stack.push(s);
            }
        }
        let push = |x: u32, stamp: &mut Vec<u32>, stack: &mut Vec<u32>| {
            if stamp[x as usize] != g {
                stamp[x as usize] = g;
                stack.push(x);
            }
        };
        while let Some(x) = stack.pop() {
            seen(x);
            if stop(x) {
                return true;
            }
            let i = x as usize;
            for &t in self.fwd.get(i) {
                push(t, &mut self.stamp, &mut stack);
            }
            for &o in self.facts.overriders.get(&x).into_iter().flatten() {
                push(o, &mut self.stamp, &mut stack);
            }
            for &b in self.facts.bound_subtypes.get(&x).into_iter().flatten() {
                push(b, &mut self.stamp, &mut stack);
            }
            let f = v.file[i];
            let module = c.module[f as usize];
            for &w in c.refs_of(x) {
                if !names.insert((module, w)) {
                    continue;
                }
                for &t in self.callables.get(&w).into_iter().flatten() {
                    let ti = t as usize;
                    if (v.dflags[ti] & FLAG_PRIVATE != 0 && v.file[ti] != f)
                        || !c.sees_file(f, v.file[ti])
                    {
                        continue;
                    }
                    push(t, &mut self.stamp, &mut stack);
                }
            }
            // Prod code runs inside its container: a member reached means its class was built.
            if c.src(x) != Source::Test && v.parent[i] >= 0 {
                push(v.parent[i] as u32, &mut self.stamp, &mut stack);
            }
            // A test class's own fields are built for every test: not what one test method
            // exercises (the coverage walk starts from all of a test class anyway).
            if is_type_kind(v.kind[i]) && c.src(x) != Source::Test {
                // Building a type runs its init blocks, constructor and initialized properties,
                // and code that holds one may call its implicit members.
                for k in v.file_nodes(f) {
                    let ki = k as usize;
                    if v.parent[ki] == x as i32
                        && (v.rides[ki]
                            || matches!(v.kind[ki], KIND_INIT | KIND_CONSTRUCTOR)
                            || v.dflags[ki] & FLAG_INITIALIZED != 0
                            || self.implicit.contains(&v.name[ki]))
                    {
                        push(k, &mut self.stamp, &mut stack);
                    }
                }
            }
        }
        false
    }
}

/// Test units: per test class, its test methods (annotated), or the class itself when it has none
/// (specs whose tests are lambdas).
fn test_units(c: &Code) -> Vec<(u32, Vec<u32>)> {
    let v = &c.v;
    let ann: FxHashSet<u32> = TEST_ANNOTATIONS
        .iter()
        .filter_map(|s| v.string_id(s))
        .collect();
    let mut roots: Vec<u32> = v
        .test_class
        .keys()
        .copied()
        .filter(|&r| c.src(r) != Source::Prod)
        .collect();
    roots.sort_unstable();
    roots
        .into_iter()
        .map(|r| {
            let methods: Vec<u32> = v
                .file_nodes(v.file[r as usize])
                .filter(|&n| {
                    n != r
                        && c.within(n, r)
                        && v.kind[n as usize] == KIND_FUN
                        && c.annotations(n).iter().any(|a| ann.contains(a))
                })
                .collect();
            (r, methods)
        })
        .collect()
}

/// Nodes of each changed file that the diff touches, or every node of an added file.
fn touched_nodes(c: &Code, repo: &Repo) -> Result<FxHashSet<u32>, String> {
    let v = &c.v;
    let changed = repo.changed()?;
    let hunks = repo.hunks();
    let mut out = FxHashSet::default();
    for path in &changed {
        let Some(&f) = v.path_file.get(path.as_str()) else {
            continue;
        };
        let ids = v.file_nodes(f);
        let hit = hunks.as_ref().and_then(|h| h.get(path)).and_then(|h| {
            let (new, old) = (repo.read(path), repo.read_base(path));
            seeds::touched(
                v,
                ids.clone(),
                h,
                new.as_deref().and_then(seeds::lines_of).as_ref(),
                old.as_deref().and_then(seeds::lines_of).as_ref(),
            )
        });
        match hit {
            Some(h) => out.extend(h),
            None => out.extend(ids),
        }
    }
    Ok(out)
}

pub fn test_reach(inp: &Inputs, kover: Option<&Path>) -> Result<TestReach, String> {
    let c = Code::new(inp.root, inp.build, inp.cfg, inp.packed, inp.dir)?;
    let v = &c.v;
    let facts = facts(&c, inp.cfg);
    let scope: Option<FxHashSet<u32>> = match inp.repo {
        Some(r) => Some(touched_nodes(&c, r)?),
        None => None,
    };
    let in_scope = |n: u32| scope.as_ref().map(|s| s.contains(&n)).unwrap_or(true);
    let mut w = Walker::new(&c, &facts);
    let is_prod = |n: u32| c.src(n) == Source::Prod && reportable(&c, n);

    // Tests that reach nothing.
    let mut nothing: Vec<TestUnit> = Vec::new();
    let mut all_starts: Vec<u32> = Vec::new();
    let unit = |n: u32, class: u32, method: Option<String>| -> TestUnit {
        let i = n as usize;
        TestUnit {
            class: c.fq(class),
            method,
            file: v.p.file_path(v.file[i] as usize).to_string(),
            start_line: v.lo[i],
            end_line: v.hi[i],
        }
    };
    for (r, methods) in test_units(&c) {
        let whole: Vec<u32> = v
            .file_nodes(v.file[r as usize])
            .filter(|&n| c.within(n, r))
            .collect();
        all_starts.extend(whole.iter().copied());
        let class_scoped = in_scope(r) || whole.iter().any(|&n| in_scope(n));
        if !class_scoped {
            continue;
        }
        if !w.walk(&whole, is_prod, |_| {}) {
            nothing.push(unit(r, r, None));
            continue;
        }
        for m in methods {
            if (in_scope(m) || in_scope(r)) && !w.walk(&[m], is_prod, |_| {}) {
                nothing.push(unit(m, r, Some(v.p.str(v.name[m as usize]).to_string())));
            }
        }
    }

    // Prod declarations no test reaches.
    let mut reached = vec![false; v.n];
    w.walk(&all_starts, |_| false, |n| reached[n as usize] = true);
    let mut unreached: Vec<Declaration> = Vec::new();
    let mut prod_in_scope: Vec<u32> = Vec::new();
    for n in 0..v.n as u32 {
        if !is_prod(n) || !in_scope(n) {
            continue;
        }
        prod_in_scope.push(n);
        if !reached[n as usize] {
            let mut k = [0u32; 3];
            inbound(&c, n, &mut k);
            unreached.push(c.record(n, k, [false; 3]));
        }
    }
    unreached.sort_by(|a, b| (&a.file, a.start_line).cmp(&(&b.file, b.start_line)));
    let coverage = match kover {
        Some(path) => kover_coverage(&c, path, &prod_in_scope)?,
        None => Vec::new(),
    };
    Ok(TestReach {
        schema: SCHEMA,
        base: inp.repo.map(|r| r.base.clone()),
        tests_reaching_nothing: nothing,
        unreached,
        coverage,
    })
}

// ------------------------------------------------------------------ Kover

/// Line counters of a Kover (JaCoCo-format) XML report, summed over each declaration's lines.
fn kover_coverage(c: &Code, path: &Path, nodes: &[u32]) -> Result<Vec<DeclCoverage>, String> {
    let v = &c.v;
    let xml = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let lines = parse_kover(&xml);
    let mut out = Vec::new();
    for &n in nodes {
        let i = n as usize;
        let f = v.file[i] as usize;
        let path = v.p.file_path(f);
        let name = path.rsplit('/').next().unwrap_or(path);
        let Some(m) = lines.get(&(v.p.file_package(f).to_string(), name.to_string())) else {
            continue;
        };
        let Some((l, b, ins)) = sum_lines(m, v.lo[i], v.hi[i]) else {
            continue;
        };
        let count = |x: [u32; 2]| CoverageCount {
            covered: x[0],
            missed: x[1],
        };
        out.push(DeclCoverage {
            fq_name: c.fq(n),
            kind: c.kind_name(n).into(),
            file: path.to_string(),
            start_line: v.lo[i],
            end_line: v.hi[i],
            lines: count(l),
            branches: count(b),
            instructions: count(ins),
        });
    }
    Ok(out)
}

/// (lines, branches, instructions) as (covered, missed) over `lo..=hi`; None when no line there
/// has counters.
fn sum_lines(
    m: &BTreeMap<u32, [u32; 4]>,
    lo: u32,
    hi: u32,
) -> Option<([u32; 2], [u32; 2], [u32; 2])> {
    let (mut l, mut b, mut ins) = ([0u32; 2], [0u32; 2], [0u32; 2]);
    let mut any = false;
    for (_, &[mi, ci, mb, cb]) in m.range(lo..=hi) {
        any = true;
        if ci > 0 {
            l[0] += 1;
        } else if mi > 0 {
            l[1] += 1;
        }
        ins[0] += ci;
        ins[1] += mi;
        b[0] += cb;
        b[1] += mb;
    }
    any.then_some((l, b, ins))
}

/// A Kover (JaCoCo-format) XML report's line counters: (package with dots, source file name) ->
/// line -> (missed instructions, covered instructions, missed branches, covered branches).
fn parse_kover(xml: &str) -> FxHashMap<(String, String), BTreeMap<u32, [u32; 4]>> {
    let mut lines: FxHashMap<(String, String), BTreeMap<u32, [u32; 4]>> = FxHashMap::default();
    let attr = |tag: &str, key: &str| -> Option<String> {
        let at = tag.find(&format!(" {key}=\""))? + key.len() + 3;
        let end = tag[at..].find('"')? + at;
        Some(tag[at..end].to_string())
    };
    let (mut pkg, mut file) = (String::new(), None::<String>);
    for raw in xml.split('<').skip(1) {
        let tag = raw.split('>').next().unwrap_or("");
        if let Some(rest) = tag.strip_prefix("package ") {
            pkg = attr(&format!(" {rest}"), "name")
                .unwrap_or_default()
                .replace('/', ".");
        } else if let Some(rest) = tag.strip_prefix("sourcefile ") {
            file = attr(&format!(" {rest}"), "name");
        } else if tag.starts_with("/sourcefile") {
            file = None;
        } else if let (Some(rest), Some(f)) = (tag.strip_prefix("line "), &file) {
            let t = format!(" {rest}");
            let num = |k: &str| attr(&t, k).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let nr = num("nr");
            if nr > 0 {
                lines
                    .entry((pkg.clone(), f.clone()))
                    .or_default()
                    .insert(nr, [num("mi"), num("ci"), num("mb"), num("cb")]);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kover_lines_sum_over_a_declaration() {
        let xml = r#"<?xml version="1.0"?><report name="r"><package name="demo/pricing">
            <class name="demo/pricing/Pricing" sourcefilename="Pricing.kt"><counter type="LINE" missed="1" covered="2"/></class>
            <sourcefile name="Pricing.kt">
              <line nr="3" mi="0" ci="4" mb="1" cb="1"/>
              <line nr="4" mi="2" ci="0" mb="0" cb="0"/>
              <line nr="9" mi="0" ci="1" mb="0" cb="0"/>
            </sourcefile></package></report>"#;
        let m = parse_kover(xml);
        let lines = &m[&("demo.pricing".to_string(), "Pricing.kt".to_string())];
        assert_eq!(lines.len(), 3);
        let (l, b, ins) = sum_lines(lines, 3, 5).unwrap();
        assert_eq!((l, b, ins), ([1, 1], [1, 1], [4, 2]));
        assert!(sum_lines(lines, 5, 8).is_none());
    }

    #[test]
    fn globs_and_headers() {
        assert!(glob("*Fake*", "FakeRepo"));
        assert!(glob("demo.*.main", "demo.app.main"));
        assert!(!glob("Fake*", "RepoFake"));
        assert_eq!(
            strip_annotations("@Suppress(\"x\") @get:Rule internal fun"),
            "  internal fun"
        );
        assert_eq!(word_at("val ids = id", "id"), Some(10));
        assert_eq!(count_words("fun save() = store.save()", "save"), 2);
    }
}

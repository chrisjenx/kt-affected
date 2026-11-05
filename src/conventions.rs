//! Convention plugins and build-logic helpers, read statically.
//!
//! A plugin build (`pluginManagement { includeBuild(..) }`, `includeBuild`, `buildSrc`) is indexed for
//! plugin ids (`gradlePlugin { id = ..; implementationClass = .. }`, precompiled `*.gradle.kts`,
//! `META-INF/gradle-plugins`), `Plugin<Project>` bodies, top-level functions and constants. Per project,
//! the plugins it applies are evaluated over tokens: `apply(plugin = ..)` chains, `"cfg"(project(..))`,
//! `add(cfg, ..)`, `configurations.matching { it.name in SET }`, `listOf(..).forEach { .. }` loops,
//! helper calls with defaulted and named list arguments, `if (path != X)` guards and the KMP targets a
//! plugin configures. What can't be resolved is over-approximated (both branches, the union of list
//! arguments) and recorded as `assumed`, or skipped with a warning naming the file and line: a dependency
//! is never silently dropped and never silently guessed.

use crate::discover::{KMP_TARGETS, Tok, close_of, ident, is_p, named_target, tokenize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Str(String),
    List(Vec<String>),
}

/// How a dependency was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Literal project path in a plugin body or loop.
    Exact,
    /// Path built from a template or a bound variable.
    Template,
    /// Inside a branch or list whose value static reading can't decide; included to stay sound.
    Assumed,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Exact => "static-exact",
            Confidence::Template => "static-template",
            Confidence::Assumed => "assumed",
        }
    }
}

#[derive(Clone, Copy)]
struct Range {
    file: usize,
    start: usize,
    end: usize,
}

struct Fun {
    params: Vec<(String, Option<Range>)>,
    body: Range,
    /// `= expr` rather than a block.
    expr: bool,
}

#[derive(Default)]
pub struct Conventions {
    files: Vec<(String, Vec<Tok>)>,
    plugins: BTreeMap<String, Range>,
    funs: BTreeMap<String, Fun>,
    consts: BTreeMap<String, Range>,
    catalog: BTreeMap<String, String>,
    /// (class, superclass) for classes without their own `apply`.
    supers: Vec<(String, String)>,
    pub inputs: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Default, Debug)]
pub struct Effects {
    pub deps: BTreeMap<(String, String), Confidence>,
    pub plugins: BTreeSet<String>,
    pub targets: BTreeSet<String>,
    /// Named target -> its kind (`jvm("desktop")`).
    pub target_kinds: BTreeMap<String, String>,
    pub js_envs: BTreeMap<String, BTreeSet<String>>,
    pub android_host_tests: bool,
    /// Deps added through `configurations.matching { .. }`: they land only where the configuration exists.
    pub if_config_exists: BTreeSet<(String, String)>,
    /// Test tasks registered by hand: (source set whose classes they run, `None` = the JVM test set; task).
    pub custom_tests: BTreeSet<(Option<String>, String)>,
    pub warnings: Vec<String>,
}

impl Effects {
    fn dep(&mut self, cfg: String, target: String, c: Confidence) {
        let e = self.deps.entry((cfg, target)).or_insert(c);
        if c < *e {
            *e = c;
        }
    }
}

const COLLECTION_CTORS: &[&str] = &[
    "listOf",
    "setOf",
    "arrayOf",
    "mutableListOf",
    "mutableSetOf",
    "listOfNotNull",
    "sortedSetOf",
];
const PASS_THROUGH: &[&str] = &[
    "toSet",
    "toList",
    "distinct",
    "sorted",
    "toSortedSet",
    "toMutableList",
    "toMutableSet",
];
/// Calls that wrap a dependency notation without naming a configuration.
const NOTATION_HELPERS: &[&str] = &[
    "project",
    "testFixtures",
    "platform",
    "enforcedPlatform",
    "mapOf",
    "listOf",
    "setOf",
    "files",
    "lib",
    "libs",
    "to",
    "plus",
    "minus",
];

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if !matches!(
                p.file_name().and_then(|n| n.to_str()),
                Some("build" | ".gradle")
            ) {
                walk(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

fn strings_after(toks: &[Tok], key: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    for i in 0..toks.len() {
        if ident(&toks[i]) == Some(key)
            && is_p(toks.get(i + 1), '=')
            && let Some(Tok::Str(s, false)) = toks.get(i + 2)
        {
            v.push((i, s.clone()));
        }
    }
    v
}

fn skip_newlines(toks: &[Tok], mut i: usize) -> usize {
    while matches!(toks.get(i), Some(Tok::Newline)) {
        i += 1;
    }
    i
}

/// Top-level comma split of `toks` (an argument list without its parens).
fn split_args(toks: &[Tok]) -> Vec<&[Tok]> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Punct('(' | '[' | '{' | '<') => depth += 1,
            Tok::Punct(')' | ']' | '}' | '>') => depth -= 1,
            Tok::Punct(',') if depth == 0 => {
                out.push(&toks[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < toks.len() {
        out.push(&toks[start..]);
    }
    out.into_iter()
        .map(trim)
        .filter(|a| !a.is_empty())
        .collect()
}

fn trim(mut t: &[Tok]) -> &[Tok] {
    while matches!(t.first(), Some(Tok::Newline)) {
        t = &t[1..];
    }
    while matches!(t.last(), Some(Tok::Newline)) {
        t = &t[..t.len() - 1];
    }
    t
}

/// `name = value` argument -> (Some(name), value tokens).
fn named(arg: &[Tok]) -> (Option<&str>, &[Tok]) {
    if let (Some(n), true, false) = (
        arg.first().and_then(ident),
        is_p(arg.get(1), '='),
        is_p(arg.get(2), '='),
    ) {
        return (Some(n), trim(&arg[2..]));
    }
    (None, arg)
}

impl Conventions {
    /// Indexes every plugin build under `root` and the root version catalog.
    pub fn load(root: &Path, included_builds: &[String]) -> Conventions {
        let mut c = Conventions::default();
        c.load_catalog(root);
        let mut dirs: Vec<String> = included_builds.to_vec();
        if root.join("buildSrc").is_dir() && !dirs.iter().any(|d| d == "buildSrc") {
            dirs.push("buildSrc".into());
        }
        let mut class_of: BTreeMap<String, String> = BTreeMap::new();
        let mut class_body: BTreeMap<String, Range> = BTreeMap::new();
        // An included build may itself be multi-project: every directory with a build script is a plugin project.
        let mut bases = Vec::new();
        for d in &dirs {
            let mut found = Vec::new();
            walk(&root.join(d), &mut found);
            let mut sub: Vec<String> = found
                .iter()
                .filter(|f| {
                    matches!(
                        f.file_name().and_then(|n| n.to_str()),
                        Some("build.gradle.kts" | "build.gradle")
                    )
                })
                .filter_map(|f| {
                    f.parent()?
                        .strip_prefix(root)
                        .ok()
                        .map(|p| p.to_string_lossy().into_owned())
                })
                .filter(|p| !p.contains("/src/"))
                .collect();
            sub.push(d.clone());
            sub.sort();
            sub.dedup();
            bases.extend(sub);
        }
        for d in &bases {
            let base = root.join(d);
            for script in ["build.gradle.kts", "build.gradle"] {
                if let Ok(text) = std::fs::read_to_string(base.join(script)) {
                    c.inputs.push(format!("{d}/{script}"));
                    let toks = tokenize(&text);
                    let ids = strings_after(&toks, "id");
                    for (at, class) in strings_after(&toks, "implementationClass") {
                        if let Some((_, id)) = ids.iter().rev().find(|(i, _)| *i < at) {
                            class_of.insert(id.clone(), class);
                        }
                    }
                }
            }
            let mut files = Vec::new();
            walk(&base.join("src/main"), &mut files);
            files.sort();
            for f in files {
                let rel = f
                    .strip_prefix(root)
                    .unwrap_or(&f)
                    .to_string_lossy()
                    .into_owned();
                let name = f
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(stem) = name.strip_suffix(".properties") {
                    if rel.contains("META-INF/gradle-plugins/")
                        && let Ok(text) = std::fs::read_to_string(&f)
                        && let Some(cls) = text
                            .lines()
                            .find_map(|l| l.trim().strip_prefix("implementation-class="))
                    {
                        class_of.insert(stem.to_string(), cls.trim().to_string());
                    }
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&f) else {
                    continue;
                };
                if let Some(stem) = name
                    .strip_suffix(".gradle.kts")
                    .or_else(|| name.strip_suffix(".gradle"))
                {
                    c.inputs.push(rel.clone());
                    let toks = tokenize(&text);
                    let pkg = package_of(&toks);
                    let id = if pkg.is_empty() {
                        stem.to_string()
                    } else {
                        format!("{pkg}.{stem}")
                    };
                    let n = toks.len();
                    c.files.push((rel, toks));
                    c.plugins.insert(
                        id,
                        Range {
                            file: c.files.len() - 1,
                            start: 0,
                            end: n,
                        },
                    );
                } else if name.ends_with(".kt") {
                    c.inputs.push(rel.clone());
                    let toks = tokenize(&text);
                    c.files.push((rel, toks));
                    let fi = c.files.len() - 1;
                    c.index_kotlin(fi, &mut class_body);
                }
            }
        }
        c.keep_shaping_funs();
        for _ in 0..4 {
            for (sub, sup) in &c.supers {
                if let Some(r) = class_body.get(sup).copied() {
                    class_body.entry(sub.clone()).or_insert(r);
                }
            }
        }
        for (id, class) in class_of {
            let simple = class.rsplit('.').next().unwrap_or(&class).to_string();
            match class_body.get(&simple) {
                Some(r) => {
                    c.plugins.insert(id, *r);
                }
                None => c.warnings.push(format!(
                    "plugin {id}: implementation class {class} not found in plugin sources"
                )),
            }
        }
        c
    }

    /// Keeps only functions that can shape the build (directly or through another kept one): a task
    /// action's `check()` is not a helper.
    fn keep_shaping_funs(&mut self) {
        const SHAPING: &[&str] = &[
            "project",
            "add",
            "apply",
            "dependencies",
            "testFixtures",
            "plugins",
            "kotlin",
            "register",
        ];
        let calls: BTreeMap<String, BTreeSet<String>> = self
            .funs
            .iter()
            .map(|(n, f)| {
                (
                    n.clone(),
                    self.toks(f.body)
                        .iter()
                        .filter_map(ident)
                        .map(str::to_string)
                        .collect(),
                )
            })
            .collect();
        let mut keep: BTreeSet<String> = calls
            .iter()
            .filter(|(_, ids)| ids.iter().any(|i| SHAPING.contains(&i.as_str())))
            .map(|(n, _)| n.clone())
            .collect();
        loop {
            let more: Vec<String> = calls
                .iter()
                .filter(|(n, ids)| !keep.contains(*n) && ids.iter().any(|i| keep.contains(i)))
                .map(|(n, _)| n.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            keep.extend(more);
        }
        let returns: BTreeSet<String> = calls
            .iter()
            .filter(|(_, ids)| ids.contains("return"))
            .map(|(n, _)| n.clone())
            .collect();
        self.funs
            .retain(|n, f| f.expr || keep.contains(n) || returns.contains(n));
    }

    fn load_catalog(&mut self, root: &Path) {
        let Ok(text) = std::fs::read_to_string(root.join("gradle/libs.versions.toml")) else {
            return;
        };
        let mut in_plugins = false;
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('[') {
                in_plugins = l == "[plugins]";
                continue;
            }
            if !in_plugins || l.starts_with('#') {
                continue;
            }
            let Some((alias, rest)) = l.split_once('=') else {
                continue;
            };
            let alias = alias.trim().trim_end_matches(".id").to_string();
            let rest = rest.trim();
            let id = if let Some(i) = rest.find("id") {
                rest[i..].split('"').nth(1).map(str::to_string)
            } else {
                rest.trim_matches('"').split(':').next().map(str::to_string)
            };
            if let Some(id) = id.filter(|s| !s.is_empty()) {
                self.catalog
                    .insert(alias.replace(['-', '_'], "."), id.clone());
                self.catalog.insert(alias, id);
            }
        }
    }

    /// `class X : Plugin<Project>` apply bodies, functions and `val`s of one Kotlin file.
    fn index_kotlin(&mut self, fi: usize, class_body: &mut BTreeMap<String, Range>) {
        let toks = &self.files[fi].1;
        let mut i = 0;
        let mut funs = Vec::new();
        let mut consts = Vec::new();
        let mut classes = Vec::new();
        let mut supers = Vec::new();
        while i < toks.len() {
            match ident(&toks[i]) {
                Some("class") => {
                    if let Some(name) = toks.get(i + 1).and_then(ident) {
                        // Header ends at its body or at a line break that doesn't continue it.
                        let mut depth = 0i32;
                        let mut end = toks.len();
                        let mut body = None;
                        for k in i + 2..toks.len() {
                            match &toks[k] {
                                Tok::Punct('(' | '<') => depth += 1,
                                Tok::Punct(')' | '>') => depth -= 1,
                                Tok::Punct('{') if depth <= 0 => {
                                    end = k;
                                    body = Some(k);
                                    break;
                                }
                                Tok::Newline if depth <= 0 => {
                                    let n = skip_newlines(toks, k);
                                    if !matches!(toks.get(n), Some(Tok::Punct(':' | ',' | '{'))) {
                                        end = k;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        let header = &toks[i..end];
                        let is_plugin = header.windows(4).any(|w| {
                            ident(&w[0]) == Some("Plugin")
                                && is_p(Some(&w[1]), '<')
                                && ident(&w[2]) == Some("Project")
                        });
                        let apply =
                            body.and_then(|open| apply_body(toks, open, close_of(toks, open)));
                        match apply {
                            Some(r) if is_plugin || header.iter().any(|t| is_p(Some(t), ':')) => {
                                classes.push((
                                    name.to_string(),
                                    Range {
                                        file: fi,
                                        start: r.0,
                                        end: r.1,
                                    },
                                ));
                            }
                            None => {
                                // `class X : BasePlugin(..)` inherits its base's apply.
                                if let Some(colon) = header.iter().position(|t| is_p(Some(t), ':'))
                                    && let Some(sup) = header[colon + 1..].iter().find_map(ident)
                                {
                                    supers.push((name.to_string(), sup.to_string()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some("fun") => {
                    // fun [<T>] [Recv.]name(params)[: Ret] { body } | = expr
                    let Some(open) =
                        (i + 1..toks.len().min(i + 12)).find(|&k| is_p(toks.get(k), '('))
                    else {
                        i += 1;
                        continue;
                    };
                    let Some(name) = toks.get(open - 1).and_then(ident).map(str::to_string) else {
                        i += 1;
                        continue;
                    };
                    let close = close_of(toks, open);
                    let mut params = Vec::new();
                    for a in split_args(&toks[open + 1..close.min(toks.len())]) {
                        let pname = a
                            .iter()
                            .position(|t| is_p(Some(t), ':'))
                            .and_then(|c| a.get(c.wrapping_sub(1)))
                            .and_then(ident);
                        let Some(pname) = pname else { continue };
                        let default = a.iter().position(|t| is_p(Some(t), '=')).map(|eq| {
                            let base = a.as_ptr() as usize;
                            let start = (base - toks.as_ptr() as usize)
                                / std::mem::size_of::<Tok>()
                                + eq
                                + 1;
                            Range {
                                file: fi,
                                start,
                                end: start + a.len() - eq - 1,
                            }
                        });
                        params.push((pname.to_string(), default));
                    }
                    let mut k = close + 1;
                    while k < toks.len()
                        && !is_p(toks.get(k), '{')
                        && !is_p(toks.get(k), '=')
                        && !matches!(toks[k], Tok::Newline)
                    {
                        k += 1;
                    }
                    let k2 = skip_newlines(toks, k);
                    if is_p(toks.get(k2), '{') {
                        let end = close_of(toks, k2);
                        funs.push((
                            name,
                            Fun {
                                params,
                                body: Range {
                                    file: fi,
                                    start: k2 + 1,
                                    end,
                                },
                                expr: false,
                            },
                        ));
                        i = end;
                    } else if is_p(toks.get(k), '=') {
                        let end = expr_end(toks, k + 1);
                        funs.push((
                            name,
                            Fun {
                                params,
                                body: Range {
                                    file: fi,
                                    start: k + 1,
                                    end,
                                },
                                expr: true,
                            },
                        ));
                        i = end;
                    }
                }
                Some("val") => {
                    if let Some(name) = toks.get(i + 1).and_then(ident) {
                        let mut k = i + 2;
                        if is_p(toks.get(k), ':') {
                            while k < toks.len()
                                && !is_p(toks.get(k), '=')
                                && !matches!(toks[k], Tok::Newline)
                            {
                                k += 1;
                            }
                        }
                        if is_p(toks.get(k), '=') && !is_p(toks.get(k + 1), '=') {
                            let end = expr_end(toks, k + 1);
                            consts.push((
                                name.to_string(),
                                Range {
                                    file: fi,
                                    start: k + 1,
                                    end,
                                },
                            ));
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
        for (n, f) in funs {
            if n != "apply" {
                self.funs.entry(n).or_insert(f);
            }
        }
        for (n, r) in consts {
            self.consts.entry(n).or_insert(r);
        }
        for (n, r) in classes {
            class_body.insert(n, r);
        }
        self.supers.extend(supers);
    }

    fn toks(&self, r: Range) -> &[Tok] {
        let t = &self.files[r.file].1;
        &t[r.start.min(t.len())..r.end.min(t.len())]
    }

    fn plugin_id(&self, s: &str) -> String {
        self.catalog
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_string())
    }

    /// Plugin ids a build script's `plugins {}` names, resolved through the version catalog.
    pub fn resolve_ids(&self, declared: &BTreeSet<String>) -> BTreeSet<String> {
        declared
            .iter()
            .map(|d| match d.strip_prefix("libs.plugins.") {
                Some(alias) => self.plugin_id(alias),
                None => self.plugin_id(d),
            })
            .collect()
    }

    /// What applying `declared` plugins to `project` and running its build script's loops and helper
    /// calls contribute. Build-script literal `project(..)` calls are left to the script parser.
    pub fn effects(
        &self,
        project: &str,
        declared: &BTreeSet<String>,
        script: &[Tok],
        script_name: &str,
    ) -> Effects {
        let mut applied = self.resolve_ids(declared);
        let mut out = Effects::default();
        // Fixpoint: an applied plugin may apply others; `withPlugin` gates read the final set.
        for _ in 0..8 {
            let before = applied.len();
            let mut round = Effects::default();
            for id in applied.clone() {
                if let Some(r) = self.plugins.get(&id) {
                    let mut run = Run::new(
                        self,
                        project,
                        &applied,
                        self.files[r.file].0.clone(),
                        &self.files[r.file].1,
                        false,
                    );
                    run.block(self.toks(*r), &Env::new(), &Ctx::default());
                    merge(&mut round, run.out);
                }
            }
            applied.extend(round.plugins.iter().map(|p| self.plugin_id(p)));
            out = round;
            if applied.len() == before {
                break;
            }
        }
        let mut run = Run::new(
            self,
            project,
            &applied,
            script_name.to_string(),
            script,
            true,
        );
        run.block(script, &Env::new(), &Ctx::default());
        merge(&mut out, run.out);
        out.plugins = applied;
        out
    }
}

fn merge(into: &mut Effects, from: Effects) {
    for ((c, t), conf) in from.deps {
        into.dep(c, t, conf);
    }
    into.plugins.extend(from.plugins);
    into.targets.extend(from.targets);
    into.target_kinds.extend(from.target_kinds);
    for (t, e) in from.js_envs {
        into.js_envs.entry(t).or_default().extend(e);
    }
    into.android_host_tests |= from.android_host_tests;
    into.if_config_exists.extend(from.if_config_exists);
    into.custom_tests.extend(from.custom_tests);
    for w in from.warnings {
        if !into.warnings.contains(&w) {
            into.warnings.push(w);
        }
    }
}

fn package_of(toks: &[Tok]) -> String {
    let Some(p) = toks.iter().position(|t| ident(t) == Some("package")) else {
        return String::new();
    };
    let mut s = String::new();
    for t in &toks[p + 1..] {
        match t {
            Tok::Ident(x) => s.push_str(x),
            Tok::Punct('.') => s.push('.'),
            _ => break,
        }
    }
    s
}

/// Body of `fun apply(target: Project) { .. }` or `= with(target) { .. }` inside a class block.
fn apply_body(toks: &[Tok], open: usize, close: usize) -> Option<(usize, usize)> {
    let mut k = open + 1;
    while k < close {
        if ident(&toks[k]) == Some("fun")
            && ident_at(toks, k + 1) == Some("apply")
            && is_p(toks.get(k + 2), '(')
        {
            let pclose = close_of(toks, k + 2);
            let b = (pclose + 1..close).find(|&j| is_p(toks.get(j), '{'))?;
            return Some((b + 1, close_of(toks, b)));
        }
        k += 1;
    }
    None
}

fn ident_at(toks: &[Tok], i: usize) -> Option<&str> {
    toks.get(i).and_then(ident)
}

/// End of an expression starting at `start`: the first newline at bracket depth 0 not followed by a
/// binary operator continuation.
fn expr_end(toks: &[Tok], start: usize) -> usize {
    let mut depth = 0i32;
    let mut k = skip_newlines(toks, start);
    while k < toks.len() {
        match &toks[k] {
            Tok::Punct('(' | '[' | '{') => depth += 1,
            Tok::Punct(')' | ']' | '}') => {
                depth -= 1;
                if depth < 0 {
                    return k;
                }
            }
            Tok::Newline if depth == 0 => {
                let n = skip_newlines(toks, k);
                if !matches!(toks.get(n), Some(Tok::Punct('+' | '-' | '.' | '?'))) {
                    return k;
                }
            }
            _ => {}
        }
        k += 1;
    }
    toks.len()
}

type Env = BTreeMap<String, Val>;

/// Marks a call whose arguments don't fit the indexed function: a different overload, not the helper.
const ARITY: &str = "\u{0}arity";

#[derive(Clone, Default)]
struct Ctx {
    /// Configurations a `configurations.matching { .. }.configureEach { }` block adds to.
    cfgs: Option<Vec<String>>,
    /// `cfgs` came from a filter over existing configurations.
    filtered: bool,
    /// Inside the Kotlin multiplatform extension.
    kmp: bool,
    /// KMP target block being configured.
    target: Option<String>,
    /// KMP source set a block configures (`commonMain { dependencies { .. } }`).
    source_set: Option<String>,
    /// Inside a loop or helper call: build-script deps found here are new to the script parser.
    dynamic: bool,
    assumed: bool,
    template: bool,
    in_plugins: bool,
}

struct Run<'a> {
    conv: &'a Conventions,
    project: &'a str,
    applied: &'a BTreeSet<String>,
    file: String,
    base: &'a [Tok],
    script: bool,
    depth: usize,
    /// `val dep = project(..)`: the notation a later `add(cfg, dep)` adds.
    notations: BTreeMap<String, (String, Confidence)>,
    out: Effects,
}

impl<'a> Run<'a> {
    fn new(
        conv: &'a Conventions,
        project: &'a str,
        applied: &'a BTreeSet<String>,
        file: String,
        base: &'a [Tok],
        script: bool,
    ) -> Self {
        Run {
            conv,
            project,
            applied,
            file,
            base,
            script,
            depth: 0,
            notations: BTreeMap::new(),
            out: Effects::default(),
        }
    }

    fn line(&self, toks: &[Tok], i: usize) -> usize {
        let base = self.base.as_ptr() as usize;
        let at = toks.as_ptr() as usize;
        if at < base || at > base + std::mem::size_of_val(self.base) {
            return 0;
        }
        let off = (at - base) / std::mem::size_of::<Tok>() + i;
        1 + self.base[..off.min(self.base.len())]
            .iter()
            .filter(|t| matches!(t, Tok::Newline))
            .count()
    }

    fn warn(&mut self, toks: &[Tok], i: usize, what: &str) {
        let w = format!("{}:{}: {what}", self.file, self.line(toks, i));
        if !self.out.warnings.contains(&w) {
            self.out.warnings.push(w);
        }
    }

    // ------------------------------------------------------------ values

    fn str_lit(&self, s: &str, interp: bool, env: &Env) -> Option<(String, bool)> {
        if !interp {
            return Some((s.to_string(), false));
        }
        let mut out = String::new();
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'$' {
                let (name, next) = if b.get(i + 1) == Some(&b'{') {
                    let end = s[i + 2..].find('}')? + i + 2;
                    (s[i + 2..end].trim().to_string(), end + 1)
                } else {
                    let mut e = i + 1;
                    while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'_') {
                        e += 1;
                    }
                    (s[i + 1..e].to_string(), e)
                };
                match self.var(&name, env)? {
                    Val::Str(v) => out.push_str(&v),
                    Val::List(_) => return None,
                }
                i = next;
            } else {
                out.push(b[i] as char);
                i += 1;
            }
        }
        Some((out, true))
    }

    fn var(&self, name: &str, env: &Env) -> Option<Val> {
        if let Some(v) = env.get(name) {
            return Some(v.clone());
        }
        match name {
            "path" | "project.path" | "target.path" => {
                return Some(Val::Str(self.project.to_string()));
            }
            "name" | "project.name" => {
                return Some(Val::Str(
                    self.project.rsplit(':').next().unwrap_or("").to_string(),
                ));
            }
            _ => {}
        }
        let r = *self.conv.consts.get(name)?;
        if self.depth > 12 {
            return None;
        }
        let mut sub = Run::new(
            self.conv,
            self.project,
            self.applied,
            self.file.clone(),
            self.base,
            self.script,
        );
        sub.depth = self.depth + 1;
        sub.value(self.conv.toks(r), &Env::new()).map(|(v, _)| v)
    }

    /// (value, built from a template or variable).
    fn value(&mut self, toks: &[Tok], env: &Env) -> Option<(Val, bool)> {
        let toks = trim(toks);
        if toks.is_empty() {
            return None;
        }
        // Binary + / - at depth 0, left to right.
        let mut depth = 0i32;
        let mut ops = Vec::new();
        for (i, t) in toks.iter().enumerate() {
            match t {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => depth -= 1,
                Tok::Punct(c @ ('+' | '-')) if depth == 0 && i > 0 => ops.push((i, *c)),
                _ => {}
            }
        }
        if !ops.is_empty() {
            let mut acc = self.value(&toks[..ops[0].0], env)?;
            for (n, (at, op)) in ops.iter().enumerate() {
                let end = ops.get(n + 1).map(|o| o.0).unwrap_or(toks.len());
                let rhs = self.value(&toks[at + 1..end], env)?;
                let tmpl = acc.1 || rhs.1;
                acc = (
                    match (acc.0, rhs.0, op) {
                        (Val::List(a), Val::List(b), '+') => {
                            Val::List(a.into_iter().chain(b).collect())
                        }
                        (Val::List(a), Val::List(b), '-') => {
                            Val::List(a.into_iter().filter(|x| !b.contains(x)).collect())
                        }
                        (Val::List(mut a), Val::Str(s), '+') => {
                            a.push(s);
                            Val::List(a)
                        }
                        (Val::List(a), Val::Str(s), '-') => {
                            Val::List(a.into_iter().filter(|x| *x != s).collect())
                        }
                        (Val::Str(a), Val::Str(b), '+') => Val::Str(a + &b),
                        _ => return None,
                    },
                    tmpl,
                );
            }
            return Some(acc);
        }
        if is_p(toks.first(), '(') && close_of(toks, 0) == toks.len() - 1 {
            return self.value(&toks[1..toks.len() - 1], env);
        }
        // x.toSet() and friends
        if toks.len() >= 4
            && is_p(toks.last(), ')')
            && is_p(toks.get(toks.len() - 2), '(')
            && let Some(m) = ident(&toks[toks.len() - 3])
            && PASS_THROUGH.contains(&m)
            && is_p(toks.get(toks.len() - 4), '.')
        {
            return self.value(&toks[..toks.len() - 4], env);
        }
        match toks {
            [Tok::Str(s, interp)] => {
                let (v, t) = self.str_lit(s, *interp, env)?;
                return Some((Val::Str(v), t));
            }
            [Tok::Ident(n)] => {
                let from_env = env.contains_key(n);
                return self.var(n, env).map(|v| (v, from_env));
            }
            [Tok::Ident(a), Tok::Punct('.'), Tok::Ident(b)] if a == "project" || a == "target" => {
                return self.var(&format!("project.{b}"), env).map(|v| (v, true));
            }
            _ => {}
        }
        if let (Some(f), true) = (toks.first().and_then(ident), is_p(toks.get(1), '(')) {
            let close = close_of(toks, 1);
            if close == toks.len() - 1 {
                let args = &toks[2..close];
                if COLLECTION_CTORS.contains(&f) {
                    let mut items = Vec::new();
                    let mut tmpl = false;
                    for a in split_args(args) {
                        match self.value(a, env)? {
                            (Val::Str(s), t) => {
                                items.push(s);
                                tmpl |= t;
                            }
                            (Val::List(l), t) => {
                                items.extend(l);
                                tmpl |= t;
                            }
                        }
                    }
                    return Some((Val::List(items), tmpl));
                }
                if matches!(f, "emptyList" | "emptySet") {
                    return Some((Val::List(vec![]), false));
                }
                if let Some(fun) = self.conv.funs.get(f) {
                    if fun.expr && self.depth < 12 {
                        let env2 = self
                            .bind(fun, args, env)
                            .filter(|e| !e.contains_key(ARITY))?;
                        let mut sub = Run::new(
                            self.conv,
                            self.project,
                            self.applied,
                            self.file.clone(),
                            self.base,
                            self.script,
                        );
                        sub.depth = self.depth + 1;
                        let body = self.conv.toks(fun.body);
                        let body = if ident(body.first()?) == Some("return") {
                            &body[1..]
                        } else {
                            body
                        };
                        return sub.value(body, &env2).map(|(v, _)| (v, true));
                    }
                    if !fun.expr && self.depth < 12 {
                        // Block body: its `return expr`.
                        let body = self.conv.toks(fun.body);
                        if let Some(r) = body.iter().position(|t| ident(t) == Some("return")) {
                            let env2 = self
                                .bind(fun, args, env)
                                .filter(|e| !e.contains_key(ARITY))?;
                            let end = expr_end(body, r + 1);
                            let mut sub = Run::new(
                                self.conv,
                                self.project,
                                self.applied,
                                self.file.clone(),
                                self.base,
                                self.script,
                            );
                            sub.depth = self.depth + 1;
                            return sub.value(&body[r + 1..end], &env2).map(|(v, _)| (v, true));
                        }
                    }
                }
            }
        }
        None
    }

    fn bind(&mut self, fun: &Fun, args: &[Tok], env: &Env) -> Option<Env> {
        let mut out = Env::new();
        let mut pos = 0;
        for a in split_args(args) {
            if a.iter().any(|t| matches!(t, Tok::Punct('{'))) && is_p(a.first(), '{') {
                continue; // trailing lambda
            }
            let (name, v) = named(a);
            let pname = match name {
                Some(n) if fun.params.iter().any(|(p, _)| p == n) => n.to_string(),
                _ => {
                    let Some(p) = fun.params.get(pos) else {
                        return Some(Env::from([(ARITY.into(), Val::Str(String::new()))]));
                    };
                    pos += 1;
                    p.0.clone()
                }
            };
            if let Some((val, _)) = self.value(v, env) {
                out.insert(pname, val);
            }
        }
        for (p, d) in &fun.params {
            if !out.contains_key(p)
                && let Some(r) = d
            {
                let toks = self.conv.toks(*r);
                if let Some((v, _)) = self.value(toks, &Env::new()) {
                    out.insert(p.clone(), v);
                }
            }
        }
        Some(out)
    }

    fn plugin_expr(&mut self, toks: &[Tok], env: &Env) -> Option<String> {
        let toks = trim(toks);
        // plugin("alias").pluginId / libs.findPlugin("alias")...pluginId
        if let Some(p) = toks
            .iter()
            .position(|t| matches!(ident(t), Some("plugin" | "findPlugin")))
            && is_p(toks.get(p + 1), '(')
            && let Some(Tok::Str(alias, false)) = toks.get(p + 2)
        {
            return Some(self.conv.plugin_id(alias));
        }
        // libs.plugins.a.b(.get())?.pluginId
        if ident_at(toks, 0) == Some("libs") && ident_at(toks, 2) == Some("plugins") {
            let mut parts = Vec::new();
            let mut k = 4;
            while let Some(n) = ident_at(toks, k) {
                if matches!(n, "get" | "pluginId" | "map") {
                    break;
                }
                parts.push(n);
                k += 2;
            }
            return Some(self.conv.plugin_id(&parts.join(".")));
        }
        match self.value(toks, env)? {
            (Val::Str(s), _) => Some(self.conv.plugin_id(&s)),
            _ => None,
        }
    }

    fn cond(&mut self, toks: &[Tok], env: &Env) -> Option<bool> {
        let toks = trim(toks);
        for (op, is_and) in [("&&", true), ("||", false)] {
            let c = op.chars().next().unwrap();
            if let Some(at) =
                (1..toks.len()).find(|&k| is_p(toks.get(k), c) && is_p(toks.get(k + 1), c))
            {
                let a = self.cond(&toks[..at], env);
                let b = self.cond(&toks[at + 2..], env);
                return match (a, b, is_and) {
                    (Some(false), _, true) | (_, Some(false), true) => Some(false),
                    (Some(true), _, false) | (_, Some(true), false) => Some(true),
                    (Some(x), Some(y), _) => Some(if is_and { x && y } else { x || y }),
                    _ => None,
                };
            }
        }
        if ident_at(toks, 0) == Some("true") && toks.len() == 1 {
            return Some(true);
        }
        if ident_at(toks, 0) == Some("false") && toks.len() == 1 {
            return Some(false);
        }
        for (k, t) in toks.iter().enumerate() {
            let negate = is_p(Some(t), '!');
            if (negate || is_p(Some(t), '=')) && is_p(toks.get(k + 1), '=') {
                let a = self.value(&toks[..k], env)?.0;
                let b = self.value(&toks[k + 2..], env)?.0;
                return Some((a == b) != negate);
            }
            if ident(t) == Some("in") && k > 0 {
                let neg = is_p(toks.get(k - 1), '!');
                let lhs_end = if neg { k - 1 } else { k };
                let a = self.value(&toks[..lhs_end], env)?.0;
                let b = self.value(&toks[k + 1..], env)?.0;
                return match (a, b) {
                    (Val::Str(s), Val::List(l)) => Some(l.contains(&s) != neg),
                    _ => None,
                };
            }
        }
        None
    }

    // ------------------------------------------------------------ statements

    fn block(&mut self, toks: &[Tok], env: &Env, ctx: &Ctx) {
        let mut guarded = ctx.clone();
        let mut ctx = ctx;
        let mut calls: Vec<(usize, Option<String>)> = Vec::new();
        let mut stmt_start = 0usize;
        let mut pending_cfgs: Option<Vec<String>> = None;
        let mut i = 0;
        while i < toks.len() {
            // `if (cond) return`: what follows runs only when cond is false.
            if ident(&toks[i]) == Some("if") && is_p(toks.get(i + 1), '(') && calls.is_empty() {
                let pc = close_of(toks, i + 1);
                if ident_at(toks, pc + 1) == Some("return") {
                    match self.cond(&toks[i + 2..pc.min(toks.len())], env) {
                        Some(true) => return,
                        Some(false) => {}
                        None => {
                            guarded.assumed = true;
                            ctx = &guarded;
                        }
                    }
                    i = pc + 2;
                    continue;
                }
            }
            match &toks[i] {
                Tok::Newline | Tok::Punct(';') => {
                    if calls.is_empty() {
                        stmt_start = i + 1;
                        pending_cfgs = None;
                    }
                }
                Tok::Punct('(') => {
                    let callee = match toks.get(i.wrapping_sub(1)) {
                        Some(Tok::Ident(s)) => Some(s.clone()),
                        Some(Tok::Str(s, false)) => Some(s.clone()),
                        _ => None,
                    };
                    // Helper call: evaluate its body with the arguments bound.
                    if let Some(name) = callee.as_deref()
                        && let Some(fun) = self.conv.funs.get(name)
                        && !fun.expr
                        && self.depth < 10
                        && !self.is_value_position(toks, stmt_start, i)
                    {
                        let close = close_of(toks, i);
                        let args = &toks[i + 1..close.min(toks.len())];
                        match self.bind(fun, args, env) {
                            Some(env2) if env2.contains_key(ARITY) => {}
                            Some(env2) => {
                                let body = self.conv.toks(fun.body);
                                let mut c2 = ctx.clone();
                                c2.dynamic = true;
                                self.depth += 1;
                                let file = std::mem::replace(
                                    &mut self.file,
                                    self.conv.files[fun.body.file].0.clone(),
                                );
                                let base = std::mem::replace(
                                    &mut self.base,
                                    &self.conv.files[fun.body.file].1,
                                );
                                self.block(body, &env2, &c2);
                                self.file = file;
                                self.base = base;
                                self.depth -= 1;
                            }
                            None => self.warn(
                                toks,
                                i,
                                &format!("call to {name}: arguments not resolvable statically"),
                            ),
                        }
                    }
                    calls.push((i, callee));
                }
                Tok::Punct(')') => {
                    calls.pop();
                }
                Tok::Punct('{') => {
                    let close = close_of(toks, i);
                    let inner = &toks[i + 1..close.min(toks.len())];
                    let label = &toks[stmt_start..i];
                    i = self.open_block(toks, i, close, inner, label, env, ctx, &mut pending_cfgs);
                    continue;
                }
                Tok::Ident(id) => {
                    let id = id.as_str();
                    if let Some(next) = self.ident(
                        toks,
                        i,
                        id,
                        env,
                        ctx,
                        &calls,
                        stmt_start,
                        pending_cfgs.as_ref(),
                    ) {
                        i = next;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// `val x = helper(..)` evaluates a value, not a body.
    fn is_value_position(&self, toks: &[Tok], stmt_start: usize, at: usize) -> bool {
        toks[stmt_start..at]
            .iter()
            .any(|t| ident(t) == Some("val") || ident(t) == Some("return"))
    }

    #[allow(clippy::too_many_arguments)]
    fn open_block(
        &mut self,
        toks: &[Tok],
        open: usize,
        close: usize,
        inner: &[Tok],
        label: &[Tok],
        env: &Env,
        ctx: &Ctx,
        pending_cfgs: &mut Option<Vec<String>>,
    ) -> usize {
        let ids: Vec<&str> = label.iter().filter_map(ident).collect();
        let last = ids.last().copied();
        let mut c = ctx.clone();
        let after = close + 1;
        // Lambda parameter: `{ p -> .. }`
        let param = match (
            inner.iter().position(|t| !matches!(t, Tok::Newline)),
            inner.len(),
        ) {
            (Some(p), _)
                if ident(&inner[p]).is_some()
                    && is_p(inner.get(p + 1), '-')
                    && is_p(inner.get(p + 2), '>') =>
            {
                Some((ident(&inner[p]).unwrap().to_string(), p + 3))
            }
            _ => None,
        };
        let (pname, body) = match &param {
            Some((n, skip)) => (n.clone(), &inner[*skip..]),
            None => ("it".to_string(), inner),
        };

        // configurations.matching { it.name in SET } -> the next configureEach/all block's configurations
        if last == Some("matching") || last == Some("filter") {
            if ids.contains(&"configurations") {
                let names = match inner.iter().position(|t| ident(t) == Some("in")) {
                    Some(k) => self
                        .value(&inner[k + 1..], env)
                        .and_then(|(v, _)| if let Val::List(l) = v { Some(l) } else { None }),
                    None => inner.iter().find_map(|t| {
                        if let Tok::Str(s, false) = t {
                            Some(vec![s.clone()])
                        } else {
                            None
                        }
                    }),
                };
                if names.is_none() {
                    self.warn(
                        toks,
                        open,
                        "configurations filter not resolvable statically",
                    );
                }
                *pending_cfgs = names;
            }
            return after;
        }
        if matches!(last, Some("configureEach" | "all")) && pending_cfgs.is_some() {
            c.cfgs = pending_cfgs.take();
            c.filtered = true;
            self.block(inner, env, &c);
            return after;
        }
        if ids.contains(&"configurations")
            && let Some(Tok::Str(s, false)) = label.iter().find(|t| matches!(t, Tok::Str(..)))
            && ids
                .iter()
                .find(|i| {
                    matches!(
                        **i,
                        "named" | "getByName" | "maybeCreate" | "create" | "register"
                    )
                })
                .is_some()
        {
            c.cfgs = Some(vec![s.clone()]);
            self.block(inner, env, &c);
            return after;
        }

        // Loops
        if last == Some("forEach") || last == Some("onEach") {
            let dot = label.iter().rposition(|t| is_p(Some(t), '.'));
            let recv = dot.map(|d| &label[..d]).unwrap_or(&[]);
            let recv = recv_expr(recv);
            match self.value(recv, env) {
                Some((Val::List(items), _)) => {
                    for item in items {
                        let mut e = env.clone();
                        e.insert(pname.clone(), Val::Str(item));
                        let mut c2 = c.clone();
                        c2.dynamic = true;
                        c2.template = true;
                        self.block(body, &e, &c2);
                    }
                }
                _ => {
                    // `listOf(js(), wasmJs()).forEach { it.browser { } }`: each KMP target in turn.
                    let calls: Vec<&str> = recv
                        .iter()
                        .filter_map(ident)
                        .filter(|i| KMP_TARGETS.contains(i))
                        .collect();
                    if ctx.kmp && !calls.is_empty() {
                        for t in calls {
                            self.out.targets.insert(t.to_string());
                            let mut c2 = c.clone();
                            c2.target = Some(t.to_string());
                            self.block(body, env, &c2);
                        }
                    } else if self.mentions_project(body) {
                        self.warn(toks, open, "loop over a list computed at configuration time; its project dependencies are not known statically");
                    }
                }
            }
            return after;
        }
        if ids.first() == Some(&"for")
            && let Some(inp) = label.iter().position(|t| ident(t) == Some("in"))
        {
            let var = label
                .get(inp.wrapping_sub(1))
                .and_then(ident)
                .unwrap_or("it")
                .to_string();
            let expr = &label[inp + 1..label.len().saturating_sub(1)];
            match self.value(expr, env) {
                Some((Val::List(items), _)) => {
                    for item in items {
                        let mut e = env.clone();
                        e.insert(var.clone(), Val::Str(item));
                        let mut c2 = c.clone();
                        c2.dynamic = true;
                        c2.template = true;
                        self.block(inner, &e, &c2);
                    }
                }
                _ => {
                    if self.mentions_project(inner) {
                        self.warn(toks, open, "loop over a list computed at configuration time; its project dependencies are not known statically");
                    }
                }
            }
            return after;
        }

        // if / else
        if ids.first() == Some(&"if") || (ids.first() == Some(&"else") && ids.get(1) == Some(&"if"))
        {
            let p = label.iter().position(|t| is_p(Some(t), '('));
            let decided = p.and_then(|p| {
                let pc = close_of(label, p);
                self.cond(&label[p + 1..pc.min(label.len())], env)
            });
            let mut k = skip_newlines(toks, after);
            let has_else = ident_at(toks, k) == Some("else");
            match decided {
                Some(true) => self.block(inner, env, &c),
                Some(false) => {}
                None => {
                    let mut c2 = c.clone();
                    c2.assumed = true;
                    self.block(inner, env, &c2);
                }
            }
            if has_else {
                k += 1;
                let k2 = skip_newlines(toks, k);
                if ident_at(toks, k2) == Some("if") {
                    // else if: the next statement is another `if` block; take it only if this branch didn't.
                    if decided == Some(true) {
                        // skip the whole else-if chain body
                        if let Some(b) = (k2..toks.len()).find(|&j| is_p(toks.get(j), '{')) {
                            return close_of(toks, b) + 1;
                        }
                    }
                    return k2;
                }
                if is_p(toks.get(k2), '{') {
                    let ec = close_of(toks, k2);
                    let else_inner = &toks[k2 + 1..ec.min(toks.len())];
                    match decided {
                        Some(false) => self.block(else_inner, env, &c),
                        Some(true) => {}
                        None => {
                            let mut c2 = c.clone();
                            c2.assumed = true;
                            self.block(else_inner, env, &c2);
                        }
                    }
                    return ec + 1;
                }
            }
            return after;
        }

        // Plugin-gated blocks
        if (matches!(last, Some("withPlugin" | "withId"))
            || ids
                .windows(2)
                .any(|w| matches!(w[0], "withPlugin" | "withId")))
            && let Some(Tok::Str(s, false)) = label.iter().find(|t| matches!(t, Tok::Str(..)))
        {
            let id = self.conv.plugin_id(s);
            if self.applied.contains(&id) || self.applied.contains(s) {
                self.block(inner, env, &c);
            }
            return after;
        }

        if let Some(task) = registered_test_task(label) {
            let ss = inner
                .iter()
                .position(|t| ident(t) == Some("testClassesDirs"))
                .and_then(|k| {
                    inner[k..]
                        .iter()
                        .take_while(|t| !matches!(t, Tok::Newline))
                        .filter_map(ident)
                        .find_map(|n| {
                            n.strip_suffix("Compilation")
                                .filter(|s| s.ends_with("Test"))
                                .map(str::to_string)
                        })
                });
            self.out.custom_tests.insert((ss, task));
        }
        if ids
            .windows(2)
            .any(|w| w[0] == "testRuns" && matches!(w[1], "create" | "register"))
            && let (Some(Tok::Str(run, false)), Some(t)) = (
                label.iter().find(|t| matches!(t, Tok::Str(..))),
                &ctx.target,
            )
        {
            let env = self
                .out
                .js_envs
                .get(t)
                .and_then(|e| e.iter().next().cloned());
            let cap = |s: &str| s[..1].to_uppercase() + &s[1..];
            let task = match env {
                Some(e) => format!("{t}{}{}Test", cap(&e), cap(run)),
                None => format!("{t}{}Test", cap(run)),
            };
            self.out
                .custom_tests
                .insert((Some(format!("{t}Test")), task));
        }
        if last == Some("plugins") {
            c.in_plugins = true;
        }
        if ids.contains(&"KotlinMultiplatformExtension")
            || (ids.as_slice() == ["kotlin"] && !ctx.kmp)
        {
            c.kmp = true;
        }
        if ctx.kmp
            && (ids.contains(&"KotlinMultiplatformAndroidLibraryTarget")
                || ids.as_slice() == ["androidLibrary"])
        {
            self.out.targets.insert("androidLibrary".into());
            c.target = Some("androidLibrary".into());
        }
        if ctx.kmp
            && let Some(t) = ids.iter().rev().find(|i| KMP_TARGETS.contains(i))
        {
            let name = label
                .iter()
                .rposition(|x| ident(x) == Some(*t))
                .and_then(|at| named_target(label, at));
            if let Some(n) = &name {
                self.out.target_kinds.insert(n.clone(), t.to_string());
            }
            let t = name.unwrap_or_else(|| t.to_string());
            self.out.targets.insert(t.clone());
            c.target = Some(t);
        }
        if let Some(env_name) = last.filter(|l| matches!(*l, "browser" | "nodejs" | "d8"))
            && let Some(t) = &ctx.target
        {
            self.out
                .js_envs
                .entry(t.clone())
                .or_default()
                .insert(env_name.to_string());
        }
        if last == Some("withHostTest") {
            self.out.android_host_tests = true;
        }
        if ctx.kmp
            && let Some(ss) = crate::discover::block_source_set(label, false, true)
        {
            c.source_set = Some(ss);
        }
        self.block(inner, env, &c);
        after
    }

    /// A `project(..)` dependency notation, not `project.path`.
    fn mentions_project(&self, toks: &[Tok]) -> bool {
        (0..toks.len()).any(|k| ident(&toks[k]) == Some("project") && is_p(toks.get(k + 1), '('))
    }

    #[allow(clippy::too_many_arguments)]
    fn ident(
        &mut self,
        toks: &[Tok],
        i: usize,
        id: &str,
        env: &Env,
        ctx: &Ctx,
        calls: &[(usize, Option<String>)],
        stmt_start: usize,
        _pending: Option<&Vec<String>>,
    ) -> Option<usize> {
        let prev_dot = is_p(toks.get(i.wrapping_sub(1)), '.');
        // Plugin application
        if id == "apply" && is_p(toks.get(i + 1), '(') && !self.script_literal(ctx) {
            let close = close_of(toks, i + 1);
            for a in split_args(&toks[i + 2..close.min(toks.len())]) {
                let (n, v) = named(a);
                if (n.is_none() || n == Some("plugin"))
                    && let Some(p) = self.plugin_expr(v, env)
                {
                    self.out.plugins.insert(p);
                }
            }
            return Some(close + 1);
        }
        if id == "apply" && ident_at(toks, i + 1) == Some("plugin") && is_p(toks.get(i + 2), ':') {
            if let Some(Tok::Str(s, false)) = toks.get(i + 3) {
                self.out.plugins.insert(self.conv.plugin_id(s));
            }
            return Some(i + 4);
        }
        if ctx.in_plugins && !self.script {
            if matches!(id, "id" | "kotlin" | "alias") && is_p(toks.get(i + 1), '(') {
                let close = close_of(toks, i + 1);
                let arg = &toks[i + 2..close.min(toks.len())];
                let p = match id {
                    "kotlin" => self.value(arg, env).and_then(|(v, _)| {
                        if let Val::Str(s) = v {
                            Some(format!("org.jetbrains.kotlin.{s}"))
                        } else {
                            None
                        }
                    }),
                    "alias" => self.plugin_expr(arg, env),
                    _ => self.plugin_expr(arg, env),
                };
                if let Some(p) = p {
                    self.out.plugins.insert(p);
                }
                return Some(close + 1);
            }
            if !prev_dot
                && !is_p(toks.get(i + 1), '(')
                && !is_p(toks.get(i + 1), '.')
                && (id.contains('-') || matches!(id, "java" | "application"))
            {
                self.out.plugins.insert(id.to_string());
            }
        }
        // KMP targets declared as calls: jvm(), iosSimulatorArm64()
        if ctx.kmp && !prev_dot && is_p(toks.get(i + 1), '(') && KMP_TARGETS.contains(&id) {
            match named_target(toks, i) {
                Some(n) => {
                    self.out.target_kinds.insert(n.clone(), id.to_string());
                    self.out.targets.insert(n);
                }
                None => {
                    self.out.targets.insert(id.to_string());
                }
            }
        }
        if ctx.kmp && id == "withHostTest" {
            self.out.android_host_tests = true;
        }
        if matches!(id, "browser" | "nodejs" | "d8")
            && is_p(toks.get(i + 1), '(')
            && let Some(t) = &ctx.target
        {
            self.out
                .js_envs
                .entry(t.clone())
                .or_default()
                .insert(id.to_string());
        }
        // Project dependency notation
        let project_call = id == "project"
            && is_p(toks.get(i + 1), '(')
            && (!prev_dot || matches!(ident_at(toks, i.wrapping_sub(2)), Some("dependencies")));
        if id == "add" && is_p(toks.get(i + 1), '(') && !self.notations.is_empty() {
            let close = close_of(toks, i + 1);
            let args = split_args(&toks[i + 2..close.min(toks.len())]);
            if args.len() >= 2 && !args[1].iter().any(|t| ident(t) == Some("project")) {
                let bound: Vec<(String, Confidence)> = args[1]
                    .iter()
                    .filter_map(ident)
                    .filter_map(|n| self.notations.get(n).cloned())
                    .collect();
                if !bound.is_empty() {
                    let cfgs = match self.value(args[0], env) {
                        Some((Val::Str(s), _)) => Some(vec![self.with_source_set(&s, ctx)]),
                        Some((Val::List(l), _)) => Some(l),
                        None => ctx.cfgs.clone(),
                    };
                    match cfgs {
                        Some(cfgs) => {
                            for (p, conf) in bound {
                                for c in &cfgs {
                                    self.out.dep(
                                        c.clone(),
                                        p.clone(),
                                        if ctx.assumed {
                                            Confidence::Assumed
                                        } else {
                                            conf
                                        },
                                    );
                                }
                            }
                        }
                        None => {
                            self.warn(toks, i, "add(..): configuration not resolvable statically")
                        }
                    }
                    return Some(close + 1);
                }
            }
        }
        if project_call {
            let close = close_of(toks, i + 1);
            if self.script && !ctx.dynamic {
                return Some(close + 1); // the script parser reads literal calls
            }
            let args = &toks[i + 2..close.min(toks.len())];
            let target = self.project_path(args, env);
            match target {
                Some((p, tmpl)) if ident_at(toks, stmt_start) == Some("val") => {
                    let name = ident_at(toks, stmt_start + 1).unwrap_or("").to_string();
                    let p = if p.starts_with(':') {
                        p
                    } else {
                        format!(":{p}")
                    };
                    let conf = if tmpl || ctx.template {
                        Confidence::Template
                    } else {
                        Confidence::Exact
                    };
                    self.notations.insert(name, (p, conf));
                }
                Some((p, tmpl)) => {
                    let cfgs = self.configs(toks, i, calls, stmt_start, env, ctx);
                    let conf = if ctx.assumed {
                        Confidence::Assumed
                    } else if tmpl || ctx.template {
                        Confidence::Template
                    } else {
                        Confidence::Exact
                    };
                    let (cfgs, conf) = match cfgs {
                        Some(c) => (c, conf),
                        None => (vec!["?".to_string()], Confidence::Assumed),
                    };
                    let p = if p.starts_with(':') {
                        p
                    } else {
                        format!(":{p}")
                    };
                    let filtered = ctx.filtered && ctx.cfgs.as_ref() == Some(&cfgs);
                    for c in cfgs {
                        if filtered {
                            self.out.if_config_exists.insert((c.clone(), p.clone()));
                        }
                        self.out.dep(c, p.clone(), conf);
                    }
                }
                None => self.warn(toks, i, "project path not resolvable statically"),
            }
            return Some(close + 1);
        }
        None
    }

    fn script_literal(&self, ctx: &Ctx) -> bool {
        self.script && !ctx.dynamic
    }

    fn project_path(&mut self, args: &[Tok], env: &Env) -> Option<(String, bool)> {
        for a in split_args(args) {
            let (n, v) = named(a);
            if let Some(n) = n
                && n != "path"
            {
                continue;
            }
            // mapOf("path" to X)
            if ident_at(v, 0) == Some("mapOf") {
                let close = close_of(v, 1);
                for pair in split_args(&v[2..close.min(v.len())]) {
                    if let (Some(Tok::Str(k, false)), Some(t)) = (
                        pair.first(),
                        pair.iter().position(|t| ident(t) == Some("to")),
                    ) && k == "path"
                    {
                        return match self.value(&pair[t + 1..], env)? {
                            (Val::Str(s), tmpl) => Some((s, tmpl)),
                            _ => None,
                        };
                    }
                }
                continue;
            }
            return match self.value(v, env)? {
                (Val::Str(s), tmpl) => Some((s, tmpl)),
                _ => None,
            };
        }
        None
    }

    /// Configurations the notation at `at` is added to.
    fn configs(
        &mut self,
        toks: &[Tok],
        _at: usize,
        calls: &[(usize, Option<String>)],
        stmt_start: usize,
        env: &Env,
        ctx: &Ctx,
    ) -> Option<Vec<String>> {
        for (open, callee) in calls.iter().rev() {
            let Some(c) = callee.as_deref() else { continue };
            if c == "add" {
                let close = close_of(toks, *open);
                let args = split_args(&toks[open + 1..close.min(toks.len())]);
                if args.len() >= 2 {
                    return match self.value(args[0], env)? {
                        (Val::Str(s), _) => Some(vec![self.with_source_set(&s, ctx)]),
                        (Val::List(l), _) => Some(l),
                    };
                }
                return ctx.cfgs.clone();
            }
            if NOTATION_HELPERS.contains(&c) {
                continue;
            }
            if self.conv.funs.contains_key(c) {
                continue;
            }
            return Some(vec![self.with_source_set(c, ctx)]);
        }
        if let Some(c) = &ctx.cfgs {
            return Some(c.clone());
        }
        // Groovy command syntax: `testImplementation project(':x')`
        toks[stmt_start..]
            .iter()
            .find_map(ident)
            .filter(|s| *s != "project")
            .map(|s| vec![self.with_source_set(s, ctx)])
    }

    fn with_source_set(&self, cfg: &str, ctx: &Ctx) -> String {
        match &ctx.source_set {
            Some(ss) if !cfg.starts_with(ss.as_str()) => {
                let mut ch = cfg.chars();
                let cap = ch
                    .next()
                    .map(|f| f.to_uppercase().collect::<String>() + ch.as_str())
                    .unwrap_or_default();
                format!("{ss}{cap}")
            }
            _ => cfg.to_string(),
        }
    }
}

/// `tasks.register<Test>("x")` / `tasks.register("x", Test::class)` -> "x".
fn registered_test_task(label: &[Tok]) -> Option<String> {
    let r = label
        .iter()
        .position(|t| matches!(ident(t), Some("register" | "create")))?;
    if !label[..r].iter().any(|t| ident(t) == Some("tasks")) {
        return None;
    }
    let rest = &label[r + 1..];
    let typed = (is_p(rest.first(), '<') && ident_at(rest, 1) == Some("Test"))
        || rest.windows(3).any(|w| {
            ident(&w[0]) == Some("Test") && is_p(Some(&w[1]), ':') && is_p(Some(&w[2]), ':')
        });
    if !typed {
        return None;
    }
    rest.iter().find_map(|t| {
        if let Tok::Str(s, false) = t {
            Some(s.clone())
        } else {
            None
        }
    })
}

/// The receiver of `.forEach`: the whole label minus a leading `val x =` / `dependencies.` qualifier.
fn recv_expr(label: &[Tok]) -> &[Tok] {
    let label = trim(label);
    // `) .forEach` after a call: take from the call's identifier.
    if is_p(label.last(), ')') {
        let mut depth = 0i32;
        for k in (0..label.len()).rev() {
            match label[k] {
                Tok::Punct(')') => depth += 1,
                Tok::Punct('(') => {
                    depth -= 1;
                    if depth == 0 {
                        let start = if k > 0 && ident(&label[k - 1]).is_some() {
                            k - 1
                        } else {
                            k
                        };
                        return &label[start..];
                    }
                }
                _ => {}
            }
        }
    }
    match label
        .iter()
        .rposition(|t| !matches!(t, Tok::Ident(_) | Tok::Punct('.')))
    {
        Some(p) => &label[p + 1..],
        None => label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::tokenize;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("kt-affected-conv-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn deps(e: &Effects) -> BTreeSet<(String, String)> {
        e.deps.keys().cloned().collect()
    }

    #[test]
    fn values_resolve_lists_arithmetic_and_expression_helpers() {
        let r = tmp("values");
        write(
            &r,
            "build-logic/src/main/kotlin/H.kt",
            "private val BASE = listOf(\":a\", \":b\", \":c\")\nprivate fun closure(base: List<String>, plus: List<String>, minus: Set<String>): List<String> {\n    return (base - minus) + plus\n}\nfun DependencyHandler.closureImpls(configuration: String = \"testImplementation\", plus: List<String> = emptyList(), minus: Set<String> = emptySet()) {\n    closure(BASE, plus, minus).forEach { path -> add(configuration, project(mapOf(\"path\" to path))) }\n}\n",
        );
        let c = Conventions::load(&r, &["build-logic".into()]);
        let script = tokenize(
            "dependencies {\n    closureImpls(plus = listOf(\":d\"), minus = setOf(\":b\"))\n}\n",
        );
        let e = c.effects(":m", &BTreeSet::new(), &script, "m/build.gradle.kts");
        assert_eq!(
            deps(&e),
            BTreeSet::from([
                ("testImplementation".into(), ":a".into()),
                ("testImplementation".into(), ":c".into()),
                ("testImplementation".into(), ":d".into())
            ])
        );
    }

    #[test]
    fn script_loops_over_literal_lists_add_each_project() {
        let r = tmp("loops");
        let c = Conventions::load(&r, &[]);
        let script = tokenize(
            "dependencies {\n    implementation(project(\":direct\"))\n    listOf(\n        \":x\",\n        \":y\",\n    ).forEach { testFixturesImplementation(testFixtures(project(it))) }\n    for (p in setOf(\":z\")) { testImplementation(project(p)) }\n}\n",
        );
        let e = c.effects(":m", &BTreeSet::new(), &script, "m/build.gradle.kts");
        assert_eq!(
            deps(&e),
            BTreeSet::from([
                ("testFixturesImplementation".into(), ":x".into()),
                ("testFixturesImplementation".into(), ":y".into()),
                ("testImplementation".into(), ":z".into())
            ]),
            "the literal :direct is the script parser's"
        );
    }

    #[test]
    fn a_loop_over_a_computed_list_warns_instead_of_guessing() {
        let r = tmp("dynamic");
        let c = Conventions::load(&r, &[]);
        let script = tokenize(
            "val mods: List<String> = run { file(\"x.json\").readLines() }\ndependencies {\n    mods.forEach { kover(project(it)) }\n}\n",
        );
        let e = c.effects(":", &BTreeSet::new(), &script, "build.gradle.kts");
        assert!(e.deps.is_empty());
        assert!(
            e.warnings.iter().any(|w| w.contains("build.gradle.kts:3")
                && w.contains("computed at configuration time")),
            "{:?}",
            e.warnings
        );
    }

    fn build(name: &str, files: &[(&str, &str)]) -> crate::discover::Build {
        let r = tmp(name);
        for (f, t) in files {
            write(&r, f, t);
        }
        crate::discover::discover(&r)
    }

    fn project<'b>(b: &'b crate::discover::Build, path: &str) -> &'b crate::discover::Project {
        b.project(path).unwrap_or_else(|| panic!("{path} missing"))
    }

    fn has(b: &crate::discover::Build, path: &str, cfg: &str, target: &str) -> bool {
        project(b, path)
            .deps
            .contains(&(cfg.to_string(), target.to_string()))
    }

    const SETTINGS: &str = "pluginManagement { includeBuild(\"build-logic\") }\ninclude(\":app\", \":testing\", \":core\", \":lib\")\n";
    const LEAVES: [(&str, &str); 3] = [
        ("testing/build.gradle.kts", ""),
        ("core/build.gradle.kts", ""),
        ("lib/build.gradle.kts", ""),
    ];

    fn with_leaves<'b>(files: &[(&'b str, &'b str)]) -> Vec<(&'b str, &'b str)> {
        let mut v = vec![("settings.gradle.kts", SETTINGS)];
        v.extend_from_slice(files);
        for l in LEAVES {
            if !files.iter().any(|(f, _)| *f == l.0) {
                v.push(l);
            }
        }
        v
    }

    #[test]
    fn a_precompiled_script_plugin_adds_its_dependencies_to_every_project_applying_it() {
        let b = build(
            "precompiled",
            &with_leaves(&[
                ("build-logic/build.gradle.kts", "plugins { `kotlin-dsl` }\n"),
                (
                    "build-logic/src/main/kotlin/conv.testing.gradle.kts",
                    "plugins { id(\"org.jetbrains.kotlin.jvm\") }\ndependencies {\n    testImplementation(project(\":testing\"))\n}\n",
                ),
                ("app/build.gradle.kts", "plugins { id(\"conv.testing\") }\n"),
            ]),
        );
        assert!(has(&b, ":app", "testImplementation", ":testing"));
        assert_eq!(
            project(&b, ":app")
                .dep_confidence
                .get("testImplementation :testing")
                .map(String::as_str),
            Some("static-exact")
        );
        assert!(
            !has(&b, ":lib", "testImplementation", ":testing"),
            "only where applied"
        );
    }

    #[test]
    fn a_binary_plugin_is_found_through_its_gradle_plugin_id_and_its_apply_body_read() {
        let b = build(
            "binary",
            &with_leaves(&[
                (
                    "build-logic/build.gradle.kts",
                    "gradlePlugin {\n    plugins {\n        register(\"conv\") {\n            id = \"conv.base\"\n            implementationClass = \"x.ConvPlugin\"\n        }\n    }\n}\n",
                ),
                (
                    "build-logic/src/main/kotlin/x/ConvPlugin.kt",
                    "package x\nclass ConvPlugin : Plugin<Project> {\n    override fun apply(target: Project) = with(target) {\n        dependencies {\n            \"implementation\"(project(\":core\"))\n        }\n        dependencies.add(\"detektPlugins\", dependencies.project(\":lib\"))\n        if (path != TESTING) {\n            configurations.matching { it.name in RUNTIME }.configureEach {\n                dependencies.add(project.dependencies.project(mapOf(\"path\" to TESTING)))\n            }\n        }\n    }\n    companion object {\n        const val TESTING = \":testing\"\n        val RUNTIME = setOf(\"testRuntimeOnly\", \"jvmTestRuntimeOnly\")\n    }\n}\n",
                ),
                (
                    "app/build.gradle.kts",
                    "plugins {\n    kotlin(\"jvm\")\n    id(\"conv.base\")\n}\n",
                ),
                (
                    "testing/build.gradle.kts",
                    "plugins {\n    kotlin(\"jvm\")\n    id(\"conv.base\")\n}\n",
                ),
            ]),
        );
        assert!(has(&b, ":app", "implementation", ":core"));
        assert!(has(&b, ":app", "detektPlugins", ":lib"));
        assert!(has(&b, ":app", "testRuntimeOnly", ":testing"));
        assert!(
            !has(&b, ":app", "jvmTestRuntimeOnly", ":testing"),
            "a JVM project has no jvmTest configurations to match"
        );
        assert!(
            !has(&b, ":testing", "testRuntimeOnly", ":testing"),
            "the path guard holds"
        );
    }

    #[test]
    fn a_version_catalog_alias_resolves_to_the_convention_plugin_it_names() {
        let b = build(
            "catalog",
            &with_leaves(&[
                (
                    "gradle/libs.versions.toml",
                    "[versions]\nk = \"2.0\"\n\n[plugins]\nconv-testing = { id = \"conv.testing\" }\n",
                ),
                (
                    "build-logic/src/main/kotlin/conv.testing.gradle.kts",
                    "dependencies {\n    \"testImplementation\"(project(\":testing\"))\n}\n",
                ),
                (
                    "app/build.gradle.kts",
                    "plugins {\n    alias(libs.plugins.conv.testing)\n}\n",
                ),
            ]),
        );
        assert!(has(&b, ":app", "testImplementation", ":testing"));
    }

    #[test]
    fn a_plugin_applied_by_another_plugin_contributes_too() {
        let b = build(
            "transitive",
            &with_leaves(&[
                (
                    "build-logic/src/main/kotlin/conv.outer.gradle.kts",
                    "apply(plugin = \"conv.middle\")\n",
                ),
                (
                    "build-logic/src/main/kotlin/conv.middle.gradle.kts",
                    "plugins { id(\"conv.leaf\") }\n",
                ),
                (
                    "build-logic/src/main/kotlin/conv.leaf.gradle.kts",
                    "pluginManager.withPlugin(\"conv.middle\") {\n    dependencies { \"implementation\"(project(\":core\")) }\n}\npluginManager.withPlugin(\"never.applied\") {\n    dependencies { \"implementation\"(project(\":lib\")) }\n}\n",
                ),
                ("app/build.gradle.kts", "plugins { id(\"conv.outer\") }\n"),
            ]),
        );
        assert!(has(&b, ":app", "implementation", ":core"));
        assert!(
            !has(&b, ":app", "implementation", ":lib"),
            "withPlugin gates on the applied set"
        );
    }

    #[test]
    fn java_test_fixtures_from_a_plugin_adds_the_fixture_configurations_deps() {
        let b = build(
            "fixtures",
            &with_leaves(&[
                (
                    "build-logic/src/main/kotlin/conv.fixtures.gradle.kts",
                    "plugins {\n    `java-library`\n    `java-test-fixtures`\n}\ndependencies {\n    \"testFixturesApi\"(project(\":core\"))\n    listOf(\":lib\", \":testing\").forEach { \"testImplementation\"(testFixtures(project(it))) }\n}\n",
                ),
                (
                    "app/build.gradle.kts",
                    "plugins { id(\"conv.fixtures\") }\n",
                ),
                ("app/src/test/kotlin/A.kt", ""),
                ("app/src/testFixtures/kotlin/F.kt", ""),
            ]),
        );
        let app = project(&b, ":app");
        assert!(has(&b, ":app", "testFixturesApi", ":core"));
        assert!(has(&b, ":app", "testImplementation", ":lib"));
        assert_eq!(
            app.dep_confidence
                .get("testImplementation :lib")
                .map(String::as_str),
            Some("static-template")
        );
        assert!(
            !app.source_sets["testFixtures"].test,
            "fixtures are not a test source set"
        );
    }

    #[test]
    fn kmp_targets_configured_in_a_convention_plugin_give_their_test_tasks() {
        let b = build(
            "kmp",
            &with_leaves(&[
                (
                    "gradle/libs.versions.toml",
                    "[plugins]\nkotlin-multiplatform = { id = \"org.jetbrains.kotlin.multiplatform\", version.ref = \"k\" }\n",
                ),
                (
                    "build-logic/src/main/kotlin/x/Kmp.kt",
                    "package x\nclass KmpPlugin : Plugin<Project> {\n    override fun apply(target: Project) = with(target) {\n        apply(plugin = plugin(\"kotlin-multiplatform\").pluginId)\n        configure<KotlinMultiplatformExtension> {\n            jvm()\n            wasmJs { browser { } }\n            listOf(iosSimulatorArm64()).forEach { it.binaries.framework { } }\n        }\n    }\n}\n",
                ),
                (
                    "build-logic/build.gradle.kts",
                    "gradlePlugin { plugins { register(\"kmp\") { id = \"conv.kmp\"; implementationClass = \"x.KmpPlugin\" } } }\n",
                ),
                ("app/build.gradle.kts", "plugins { id(\"conv.kmp\") }\n"),
                ("app/src/commonTest/kotlin/T.kt", ""),
            ]),
        );
        let app = project(&b, ":app");
        assert_eq!(
            app.targets,
            BTreeSet::from(["iosSimulatorArm64".into(), "jvm".into(), "wasmJs".into()])
        );
        assert_eq!(
            app.test_tasks["commonTest"],
            BTreeSet::from([
                "iosSimulatorArm64Test".into(),
                "jvmTest".into(),
                "wasmJsBrowserTest".into()
            ])
        );
    }

    #[test]
    fn a_custom_hierarchy_group_runs_on_its_member_targets_even_without_their_directories() {
        let b = build(
            "group",
            &with_leaves(&[
                (
                    "app/build.gradle.kts",
                    "plugins { kotlin(\"multiplatform\") }\nkotlin {\n    applyDefaultHierarchyTemplate {\n        group(\"nonWeb\") {\n            withJvm()\n            withIos()\n        }\n    }\n    jvm()\n    iosSimulatorArm64()\n    wasmJs { browser() }\n}\n",
                ),
                ("app/src/nonWebTest/kotlin/T.kt", ""),
            ]),
        );
        assert_eq!(
            project(&b, ":app").test_tasks["nonWebTest"],
            BTreeSet::from(["iosSimulatorArm64Test".into(), "jvmTest".into()])
        );
    }

    #[test]
    fn a_hand_registered_test_task_runs_where_its_compilation_runs() {
        let b = build(
            "custom-task",
            &with_leaves(&[
                (
                    "app/build.gradle.kts",
                    "plugins { kotlin(\"multiplatform\") }\nkotlin {\n    jvm()\n    wasmJs { browser { testRuns.create(\"capture\") { } } }\n}\nval jvmTestCompilation = kotlin.targets.getByName(\"jvm\").compilations.getByName(\"test\")\ntasks.register<Test>(\"screenshots\") {\n    testClassesDirs = jvmTestCompilation.output.classesDirs\n}\n",
                ),
                ("app/src/jvmTest/kotlin/T.kt", ""),
                ("app/src/wasmJsTest/kotlin/T.kt", ""),
            ]),
        );
        let app = project(&b, ":app");
        assert!(
            app.test_tasks["jvmTest"].contains("screenshots"),
            "{:?}",
            app.test_tasks
        );
        assert!(
            app.test_tasks["wasmJsTest"].contains("wasmJsBrowserCaptureTest"),
            "{:?}",
            app.test_tasks
        );
        assert!(!app.test_tasks["wasmJsTest"].contains("screenshots"));
    }
}

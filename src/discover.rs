//! Gradle / KMP build discovery without running Gradle.
//!
//! Reads `settings.gradle(.kts)` and every project's build script with a small
//! tokenizer shared by the Kotlin and Groovy DSLs, so a call's configuration
//! (`implementation(project(":x"))`) and its KMP source set
//! (`commonMain.dependencies { ... }`) come from the surrounding structure.
//! Anything computed at configuration time (convention plugins, loops over
//! directories) is invisible statically; `--gradle-export` asks Gradle itself.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    /// Gradle path, `:` for the root.
    pub path: String,
    /// Directory relative to the build root, `""` for the root.
    pub dir: String,
    pub build_file: Option<String>,
    /// (configuration, target project path)
    pub deps: BTreeSet<(String, String)>,
    pub source_sets: BTreeMap<String, SourceSet>,
    pub targets: BTreeSet<String>,
    /// Test source set -> Gradle test tasks that run it.
    pub test_tasks: BTreeMap<String, BTreeSet<String>>,
    /// Created by Gradle as a parent of an included path, not included itself.
    pub implicit: bool,
    /// "cfg target" -> how a dependency found through a convention plugin, loop or helper was read
    /// (static-exact, static-template, assumed). Literal build-script deps are absent (exact).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dep_confidence: BTreeMap<String, String>,
    /// Test task -> the class filter its build script sets (`filter.includeTestsMatching(..)`): a
    /// class it excludes never runs there. Absent when the task has none, or one not read statically.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub test_filters: BTreeMap<String, TestFilter>,
    /// Source directory (relative to `dir`) -> source set, for a layout other than `src/<set>/`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_dirs: BTreeMap<String, String>,
}

/// The source set a path relative to its project directory lies in: `src/<set>/..`, or under a
/// directory of the project's `source_dirs`.
pub(crate) fn source_set_at<'p>(
    source_dirs: &'p BTreeMap<String, String>,
    rest: &'p str,
) -> Option<&'p str> {
    for (d, set) in source_dirs {
        if rest.len() > d.len() && rest.starts_with(d.as_str()) && rest.as_bytes()[d.len()] == b'/'
        {
            return Some(set);
        }
    }
    let mut parts = rest.splitn(3, '/');
    let (a, b, c) = (parts.next()?, parts.next()?, parts.next());
    (a == "src" && c.is_some()).then_some(b)
}

/// A Test task's `filter { }` patterns, as Gradle's `includeTestsMatching` / `excludeTestsMatching`
/// read them: `*` matches any run of characters, dots included; a pattern starting with an upper-case
/// letter matches the simple class name.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestFilter {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

impl TestFilter {
    /// Can some test of class `fq` run under this filter? A pattern naming a method of `fq`
    /// (`pkg.Cls.method`, `pkg.Cls.*`) admits the class; only a pattern matching the class itself
    /// excludes it.
    pub fn admits(&self, fq: &str) -> bool {
        let simple = fq.rsplit('.').next().unwrap_or(fq);
        let target = |p: &str| {
            if p.starts_with(|c: char| c.is_ascii_uppercase()) {
                vec![fq, simple]
            } else {
                vec![fq]
            }
        };
        let included = self.include.is_empty()
            || self.include.iter().any(|p| {
                target(p).into_iter().any(|t| {
                    let (full, prefix) = glob(p, &format!("{t}."));
                    prefix || full || glob(p, t).0
                })
            });
        included
            && !self
                .exclude
                .iter()
                .any(|p| target(p).into_iter().any(|t| glob(p, t).0))
    }
}

/// Matches `text` against a `*` glob: (the whole pattern matches all of it, some prefix of the
/// pattern matches all of it so a longer text starting with `text` may match).
fn glob(pat: &str, text: &str) -> (bool, bool) {
    let p: Vec<char> = pat.chars().collect();
    // Pattern positions reachable after consuming the text so far (a `*` may absorb nothing).
    let close = |set: &mut Vec<bool>| {
        for i in 0..p.len() {
            if set[i] && p[i] == '*' {
                set[i + 1] = true;
            }
        }
    };
    let mut at = vec![false; p.len() + 1];
    at[0] = true;
    close(&mut at);
    for c in text.chars() {
        let mut next = vec![false; p.len() + 1];
        for i in 0..p.len() {
            if at[i] {
                if p[i] == '*' {
                    next[i] = true;
                } else if p[i] == c {
                    next[i + 1] = true;
                }
            }
        }
        close(&mut next);
        if !next.iter().any(|&b| b) {
            return (false, false);
        }
        at = next;
    }
    (at[p.len()], true)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSet {
    pub test: bool,
    pub depends_on: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    pub root_name: String,
    pub projects: Vec<Project>,
    pub included_builds: Vec<String>,
    /// Inputs whose change invalidates discovery (settings, build scripts).
    pub inputs: Vec<String>,
    pub warnings: Vec<String>,
    /// Each included build, discovered the same way. Selection runs their tests; the graph's
    /// module visibility never reads them, so linking is as if they were not modelled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub included: Vec<Included>,
}

/// An included build (`includeBuild("dir")`) and its projects, renamed into the composite's
/// namespace: its root project is `:<name>`, a subproject `:a` of it `:<name>:a`, and every `dir`
/// is relative to the outer build's root. `:<name>:a:test` is how the outer build runs its tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Included {
    /// Its directory, relative to the outer build's root.
    pub dir: String,
    /// Its build name: the directory's name unless `includeBuild(..) { name = ".." }` sets one.
    pub name: String,
    pub projects: Vec<Project>,
}

impl Build {
    /// A project of this build or, by its composite path, of an included build.
    pub fn project(&self, path: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.path == path).or_else(|| {
            self.included
                .iter()
                .flat_map(|i| &i.projects)
                .find(|p| p.path == path)
        })
    }

    /// Projects whose sources live in their own directory, longest directory first,
    /// so a file maps to its innermost project.
    pub fn source_modules(&self) -> Vec<&Project> {
        let mut v: Vec<&Project> = self
            .projects
            .iter()
            .filter(|p| !p.implicit || p.build_file.is_some())
            .collect();
        v.sort_by(|a, b| b.dir.len().cmp(&a.dir.len()).then(a.path.cmp(&b.path)));
        v
    }

    /// Project path -> projects it may reference (itself + transitive deps).
    pub fn visibility(&self) -> BTreeMap<String, BTreeSet<String>> {
        let deps: BTreeMap<&str, Vec<&str>> = self
            .projects
            .iter()
            .map(|p| {
                (
                    p.path.as_str(),
                    p.deps
                        .iter()
                        .filter(|(c, _)| puts_code_on_classpath(c))
                        .map(|(_, t)| t.as_str())
                        .collect(),
                )
            })
            .collect();
        let mut out = BTreeMap::new();
        for p in &self.projects {
            let mut seen: BTreeSet<String> = BTreeSet::from([p.path.clone()]);
            let mut q = vec![p.path.as_str()];
            while let Some(cur) = q.pop() {
                for &d in deps.get(cur).into_iter().flatten() {
                    if seen.insert(d.to_string()) {
                        q.push(d);
                    }
                }
            }
            out.insert(p.path.clone(), seen);
        }
        out
    }
}

/// Tool configurations (static analysis, coverage, lint) never put a project's classes in reach of sources.
pub fn puts_code_on_classpath(config: &str) -> bool {
    let c = config.to_ascii_lowercase();
    !(c.contains("detekt") || c.starts_with("kover") || c.starts_with("lint"))
}

/// Replaces static deps with the export's exact ones and drops predicted test tasks Gradle lacks.
pub fn merge_export(stat: &mut Build, exp: &Build) {
    for p in stat.projects.iter_mut() {
        let Some(e) = exp.project(&p.path) else {
            continue;
        };
        p.deps = e
            .deps
            .iter()
            .filter(|(_, t)| *t != p.path)
            .cloned()
            .collect();
        for (name, ss) in &e.source_sets {
            if let Some(mine) = p.source_sets.get_mut(name)
                && !ss.depends_on.is_empty()
            {
                mine.depends_on = ss.depends_on.clone();
            }
        }
        if let Some(actual) = e.test_tasks.get("*") {
            let kmp = e.source_sets.contains_key("commonMain")
                || p.source_sets.contains_key("commonMain")
                || p.source_sets.contains_key("commonTest");
            if kmp {
                // Leaf test source set per real task, then every test source set it (transitively) depends on.
                let leaves: Vec<(&str, &String)> = actual
                    .iter()
                    .filter_map(|t| leaf_of_task(t).map(|l| (l, t)))
                    .collect();
                let names: Vec<String> = p
                    .source_sets
                    .iter()
                    .filter(|(_, ss)| ss.test)
                    .map(|(n, _)| n.clone())
                    .collect();
                // The export's hierarchy covers source sets without a directory (`iosSimulatorArm64Test`).
                let mut hier = e.source_sets.clone();
                for (n, ss) in &p.source_sets {
                    hier.entry(n.clone()).or_insert_with(|| ss.clone());
                }
                for name in names {
                    let tasks: BTreeSet<String> = leaves
                        .iter()
                        .filter(|(leaf, _)| {
                            *leaf == name || depends_transitively(leaf, &name, &hier)
                        })
                        .map(|(_, t)| (*t).clone())
                        .collect();
                    p.test_tasks.insert(name, tasks);
                }
            } else {
                for tasks in p.test_tasks.values_mut() {
                    tasks.retain(|t| actual.contains(t));
                }
            }
        }
    }
    stat.warnings
        .push("project deps and test tasks from the Gradle export".into());
}

/// Leaf KMP test source set a test task runs: `jvmTest` -> jvmTest, `wasmJsBrowserTest` -> wasmJsTest,
/// `testAndroidHostTest` -> androidHostTest, `testDebugUnitTest` -> androidUnitTest.
fn leaf_of_task(task: &str) -> Option<&'static str> {
    Some(match task {
        "jvmTest" => "jvmTest",
        "testAndroidHostTest" => "androidHostTest",
        "testDebugUnitTest" => "androidUnitTest",
        "jsBrowserTest" | "jsNodeTest" => "jsTest",
        "wasmJsBrowserTest" | "wasmJsNodeTest" | "wasmJsD8Test" => "wasmJsTest",
        "wasmWasiNodeTest" => "wasmWasiTest",
        "iosSimulatorArm64Test" => "iosSimulatorArm64Test",
        "iosX64Test" => "iosX64Test",
        "macosArm64Test" => "macosArm64Test",
        "macosX64Test" => "macosX64Test",
        "linuxX64Test" => "linuxX64Test",
        "mingwX64Test" => "mingwX64Test",
        "tvosSimulatorArm64Test" => "tvosSimulatorArm64Test",
        "watchosSimulatorArm64Test" => "watchosSimulatorArm64Test",
        _ => return None,
    })
}

/// The export under `<root>/build/kt-affected/export`, if every file is newer than every discovery input.
pub fn fresh_export(root: &Path, inputs: &[String]) -> Option<(Build, Vec<String>)> {
    let dir = root.join("build/kt-affected/export");
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let newest_input = inputs.iter().filter_map(|i| mtime(&root.join(i))).max()?;
    let mut files = Vec::new();
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        if mtime(&e.path())? < newest_input {
            return None;
        }
        files.push(format!(
            "build/kt-affected/export/{}",
            e.file_name().to_string_lossy()
        ));
    }
    if files.is_empty() {
        return None;
    }
    files.sort();
    Some((read_export(root, &dir).ok()?, files))
}

pub fn is_test_source_set(name: &str) -> bool {
    // `testPaid`, `androidTestDebug`: an Android variant's own test sets; `testFixtures` is not a test.
    let variant = |p: &str| {
        name.strip_prefix(p)
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_uppercase()))
    };
    name == "test"
        || name.ends_with("Test")
        || name.ends_with("test")
        || ((variant("test") || variant("androidTest")) && !name.starts_with("testFixtures"))
}

// ------------------------------------------------------------------ tokenizer

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Ident(String),
    /// String literal body; `true` when it interpolates.
    Str(String, bool),
    Punct(char),
    Newline,
}

pub(crate) fn tokenize(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            out.push(Tok::Newline);
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if c == b'"' || c == b'\'' {
            let triple = b.get(i + 1) == Some(&c) && b.get(i + 2) == Some(&c);
            let q = c;
            i += if triple { 3 } else { 1 };
            let start = i;
            let mut interp = false;
            while i < b.len() {
                if triple {
                    if b[i] == q && b.get(i + 1) == Some(&q) && b.get(i + 2) == Some(&q) {
                        break;
                    }
                } else if b[i] == b'\\' {
                    i += 2;
                    continue;
                } else if b[i] == q || b[i] == b'\n' {
                    break;
                }
                if b[i] == b'$' && q == b'"' {
                    interp = true;
                }
                i += 1;
            }
            let end = i.min(b.len());
            out.push(Tok::Str(src[start..end].to_string(), interp));
            i = end + if triple { 3 } else { 1 };
        } else if c == b'`' {
            let start = i + 1;
            i += 1;
            while i < b.len() && b[i] != b'`' && b[i] != b'\n' {
                i += 1;
            }
            out.push(Tok::Ident(src[start..i.min(b.len())].to_string()));
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push(Tok::Ident(src[start..i].to_string()));
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
        } else {
            out.push(Tok::Punct(c as char));
            i += 1;
        }
    }
    out
}

pub(crate) fn ident(t: &Tok) -> Option<&str> {
    if let Tok::Ident(s) = t { Some(s) } else { None }
}

pub(crate) fn is_p(t: Option<&Tok>, c: char) -> bool {
    matches!(t, Some(Tok::Punct(p)) if *p == c)
}

/// Index of the token closing the bracket opened at `open`.
pub(crate) fn close_of(toks: &[Tok], open: usize) -> usize {
    let (o, c) = match toks[open] {
        Tok::Punct('(') => ('(', ')'),
        Tok::Punct('{') => ('{', '}'),
        Tok::Punct('[') => ('[', ']'),
        _ => return open,
    };
    let mut depth = 0i32;
    for (i, t) in toks.iter().enumerate().skip(open) {
        if let Tok::Punct(p) = t {
            if *p == o {
                depth += 1;
            } else if *p == c {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
        }
    }
    toks.len()
}

// ------------------------------------------------------------------ settings

struct Settings {
    root_name: Option<String>,
    includes: Vec<(String, bool)>, // (path, flat)
    project_dirs: BTreeMap<String, String>,
    included_builds: Vec<String>,
    warnings: Vec<String>,
    /// An `include` takes a computed argument: the projects it adds are found on disk.
    dynamic: bool,
    /// Every plain string literal in the script.
    literals: BTreeSet<String>,
}

fn strip_dir_prefix(s: &str) -> String {
    let mut s = s.to_string();
    for p in [
        "${rootDir}/",
        "$rootDir/",
        "${settingsDir}/",
        "$settingsDir/",
        "${rootProject.projectDir}/",
    ] {
        if let Some(r) = s.strip_prefix(p) {
            s = r.to_string();
        }
    }
    s.trim_start_matches("./").trim_end_matches('/').to_string()
}

/// Builds a settings script includes inside `pluginManagement { .. }`: plugin builds.
pub(crate) fn plugin_management_builds(settings: &str) -> BTreeSet<String> {
    let toks = tokenize(settings);
    let mut out = BTreeSet::new();
    for i in 0..toks.len() {
        if ident(&toks[i]) != Some("pluginManagement") || !is_p(toks.get(i + 1), '{') {
            continue;
        }
        let end = close_of(&toks, i + 1);
        for j in i + 2..end {
            if ident(&toks[j]) == Some("includeBuild") {
                let k = if is_p(toks.get(j + 1), '(') {
                    j + 2
                } else {
                    j + 1
                };
                if let Some(Tok::Str(v, _)) = toks.get(k) {
                    out.insert(strip_dir_prefix(v));
                }
            }
        }
    }
    out
}

fn norm_path(p: &str) -> String {
    if p.starts_with(':') {
        p.to_string()
    } else {
        format!(":{p}")
    }
}

fn parse_settings(text: &str) -> Settings {
    let toks = tokenize(text);
    let mut s = Settings {
        root_name: None,
        includes: Vec::new(),
        project_dirs: BTreeMap::new(),
        included_builds: Vec::new(),
        warnings: Vec::new(),
        dynamic: false,
        literals: toks
            .iter()
            .filter_map(|t| match t {
                Tok::Str(v, false) => Some(v.clone()),
                _ => None,
            })
            .collect(),
    };
    let mut i = 0;
    while i < toks.len() {
        match ident(&toks[i]) {
            Some(kw @ ("include" | "includeFlat")) => {
                let flat = kw == "includeFlat";
                let (start, end) = if is_p(toks.get(i + 1), '(') {
                    (i + 2, close_of(&toks, i + 1))
                } else {
                    // Groovy command form: strings separated by commas, a trailing comma continues the line.
                    let mut j = i + 1;
                    while j < toks.len() {
                        if toks[j] == Tok::Newline && !is_p(toks.get(j.wrapping_sub(1)), ',') {
                            break;
                        }
                        j += 1;
                    }
                    (i + 1, j)
                };
                let mut dynamic = false;
                for t in &toks[start..end.min(toks.len())] {
                    match t {
                        Tok::Str(v, false) => s.includes.push((norm_path(v), flat)),
                        Tok::Str(_, true) | Tok::Ident(_) => dynamic = true,
                        _ => {}
                    }
                }
                if dynamic && !s.dynamic {
                    s.dynamic = true;
                    s.warnings.push(format!(
                        "{kw}: non-literal argument; the projects it adds are taken from directories holding a build script"
                    ));
                }
                i = end;
            }
            Some("includeBuild") => {
                let j = if is_p(toks.get(i + 1), '(') {
                    i + 2
                } else {
                    i + 1
                };
                if let Some(Tok::Str(v, _)) = toks.get(j) {
                    s.included_builds.push(strip_dir_prefix(v));
                }
                i = j;
            }
            Some("rootProject")
                if is_p(toks.get(i + 1), '.')
                    && ident_at(&toks, i + 2) == Some("name")
                    && is_p(toks.get(i + 3), '=') =>
            {
                if let Some(Tok::Str(v, _)) = toks.get(i + 4) {
                    s.root_name = Some(v.clone());
                }
                i += 5;
            }
            Some("project") if is_p(toks.get(i + 1), '(') => {
                // project(":x").projectDir = file("p") | File(rootDir, "p") | new File(settingsDir, "p")
                let close = close_of(&toks, i + 1);
                let path = match toks.get(i + 2) {
                    Some(Tok::Str(v, _)) => Some(norm_path(v)),
                    _ => None,
                };
                if let Some(path) = path
                    && is_p(toks.get(close + 1), '.')
                    && ident_at(&toks, close + 2) == Some("projectDir")
                    && is_p(toks.get(close + 3), '=')
                {
                    let rhs = &toks[close + 4..(close + 12).min(toks.len())];
                    if let Some(dir) = rhs.iter().find_map(|t| {
                        if let Tok::Str(v, _) = t {
                            Some(v.clone())
                        } else {
                            None
                        }
                    }) {
                        s.project_dirs.insert(path, strip_dir_prefix(&dir));
                    }
                }
                i = close + 1;
            }
            Some("buildFileName") => {
                s.warnings
                    .push("buildFileName is customised; build scripts may be missed".into());
                i += 1;
            }
            _ => i += 1,
        }
    }
    s
}

/// Directory names a project scan never descends into: sources, outputs, tooling.
fn scan_skips(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "src" | "build" | "buildSrc" | "gradle" | "node_modules" | "out"
        )
}

/// A settings script computes its includes (a directory walk, a project DSL): every directory below the
/// root holding a build script, outside nested and included builds, is taken as a project. Gradle names
/// a project after its directory path (`:a:b` for `a/b`) unless the script names it otherwise; the
/// scheme the build scripts' own project references resolve to (`project(":x")`, `projects.x`) is the
/// one used, so a build naming projects by their leaf directory (`:b` in `a/b`) is read as such.
/// Returns every directory listed, for the discovery stamp.
fn add_scanned_projects(root: &Path, s: &mut Settings, warnings: &mut Vec<String>) -> Vec<String> {
    let builds: BTreeSet<&str> = s.included_builds.iter().map(String::as_str).collect();
    let mut found: Vec<String> = Vec::new();
    let mut listed = vec![String::new()];
    let mut stack = vec![String::new()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(root.join(&dir)) else {
            continue;
        };
        let mut children: Vec<String> = rd
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| !scan_skips(n))
            .map(|n| {
                if dir.is_empty() {
                    n
                } else {
                    format!("{dir}/{n}")
                }
            })
            .filter(|d| !builds.contains(d.as_str()))
            .collect();
        children.sort();
        for d in children.into_iter().rev() {
            if script_in(root, &d, "settings").is_some() {
                continue;
            }
            if script_in(root, &d, "build").is_some() {
                found.push(d.clone());
            }
            listed.push(d.clone());
            stack.push(d);
        }
    }
    found.sort();
    let known_dirs: BTreeSet<String> = s
        .includes
        .iter()
        .map(|(p, _)| {
            s.project_dirs
                .get(p)
                .cloned()
                .unwrap_or_else(|| p.trim_start_matches(':').replace(':', "/"))
        })
        .collect();
    found.retain(|d| !known_dirs.contains(d));
    let leaf = |d: &str| d.rsplit('/').next().unwrap_or(d).to_string();
    // A settings DSL naming most of them lists its projects: a build script it never names is a
    // directory the build no longer includes.
    let named_count = found
        .iter()
        .filter(|d| s.literals.contains(&leaf(d)) || s.literals.contains(*d))
        .count();
    if named_count * 2 > found.len() && named_count < found.len() {
        warnings.push(format!(
            "settings: {} directories holding a build script are never named by settings; not taken as projects",
            found.len() - named_count
        ));
        found.retain(|d| s.literals.contains(&leaf(d)) || s.literals.contains(d));
    }
    let nested: Vec<(String, String)> = found
        .iter()
        .map(|d| (format!(":{}", d.replace('/', ":")), d.clone()))
        .collect();
    let leaves: BTreeSet<String> = found.iter().map(|d| leaf(d)).collect();
    let flat: Option<Vec<(String, String)>> = (leaves.len() == found.len()).then(|| {
        found
            .iter()
            .map(|d| (format!(":{}", leaf(d)), d.clone()))
            .collect()
    });
    let mut refs: Vec<Vec<String>> = Vec::new();
    let mut strs: BTreeSet<String> = BTreeSet::new();
    for d in std::iter::once(String::new()).chain(found.iter().cloned()) {
        let Some(text) = script_in(root, &d, "build").and_then(|f| read(root, &f)) else {
            continue;
        };
        let toks = tokenize(&text);
        for (i, t) in toks.iter().enumerate() {
            match t {
                Tok::Str(v, false) if v.starts_with(':') && v.len() > 1 => {
                    strs.insert(v.clone());
                }
                Tok::Ident(id) if id == "projects" && is_p(toks.get(i + 1), '.') => {
                    let mut chain = Vec::new();
                    let mut j = i + 1;
                    while is_p(toks.get(j), '.')
                        && let Some(n) = ident_at(&toks, j + 1)
                    {
                        chain.push(n.to_string());
                        j += 2;
                    }
                    refs.push(chain);
                }
                _ => {}
            }
        }
    }
    let score = |scheme: &[(String, String)]| -> usize {
        let paths: BTreeSet<&str> = scheme.iter().map(|(p, _)| p.as_str()).collect();
        let accessors: BTreeSet<Vec<String>> = scheme
            .iter()
            .map(|(p, _)| {
                p.trim_start_matches(':')
                    .split(':')
                    .map(accessor_name)
                    .collect()
            })
            .collect();
        let by_accessor = refs
            .iter()
            .filter(|c| {
                accessors.contains(*c) || (c.len() > 1 && accessors.contains(&c[..c.len() - 1]))
            })
            .count();
        by_accessor + strs.iter().filter(|v| paths.contains(v.as_str())).count()
    };
    let (chosen, how) = match flat {
        Some(f) if score(&f) > score(&nested) => (f, "leaf directory name"),
        _ => (nested, "directory path"),
    };
    if !chosen.is_empty() {
        warnings.push(format!(
            "settings: {} projects taken from directories holding a build script, named by {how}",
            chosen.len()
        ));
    }
    for (p, d) in chosen {
        if how != "directory path" {
            s.project_dirs.insert(p.clone(), d);
        }
        s.includes.push((p, false));
    }
    listed
}

pub(crate) fn ident_at(toks: &[Tok], i: usize) -> Option<&str> {
    toks.get(i).and_then(ident)
}

// ------------------------------------------------------------------ build scripts

/// `foo-bar_baz` -> `fooBarBaz` (Gradle's typesafe project accessor naming).
pub fn accessor_name(segment: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for (i, c) in segment.chars().enumerate() {
        if c == '-' || c == '_' || c == '.' || c == ' ' {
            up = true;
        } else if up && i > 0 {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
            up = false;
        }
    }
    out
}

pub(crate) const KMP_TARGETS: &[&str] = &[
    "jvm",
    "androidTarget",
    "androidLibrary",
    "js",
    "wasmJs",
    "wasmWasi",
    "iosArm64",
    "iosX64",
    "iosSimulatorArm64",
    "macosArm64",
    "macosX64",
    "tvosArm64",
    "tvosX64",
    "tvosSimulatorArm64",
    "watchosArm32",
    "watchosArm64",
    "watchosX64",
    "watchosSimulatorArm64",
    "watchosDeviceArm64",
    "linuxX64",
    "linuxArm64",
    "mingwX64",
    "androidNativeArm32",
    "androidNativeArm64",
    "androidNativeX86",
    "androidNativeX64",
];

/// `jvm("desktop")`: the name a target call at `at` (a KMP target identifier) gives its target.
pub(crate) fn named_target(toks: &[Tok], at: usize) -> Option<String> {
    if !is_p(toks.get(at + 1), '(') {
        return None;
    }
    match (toks.get(at + 2), toks.get(at + 3)) {
        (Some(Tok::Str(n, false)), Some(Tok::Punct(')' | ','))) if !n.is_empty() => Some(n.clone()),
        _ => None,
    }
}

struct ScriptFacts {
    deps: BTreeSet<(String, String)>,
    targets: BTreeSet<String>,
    /// Named target -> its kind (`jvm("desktop")`: desktop -> jvm). A target absent here is its kind.
    target_kinds: BTreeMap<String, String>,
    depends_on: BTreeMap<String, BTreeSet<String>>,
    plugins: BTreeSet<String>,
    js_envs: BTreeMap<String, BTreeSet<String>>, // target -> {browser, nodejs, d8}
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Source set a `{` block names: `commonMain.dependencies {`, and under `sourceSets`:
/// `val x by getting {`, `getByName("x") {`, `x.configure {`, `x {`.
pub(crate) fn block_source_set(
    label: &[Tok],
    parent_is_ss: bool,
    inside_ss: bool,
) -> Option<String> {
    let ids: Vec<&str> = label.iter().filter_map(ident).collect();
    if let [.., ss, "dependencies"] = ids.as_slice()
        && label.len() >= 3
        && is_p(label.get(label.len() - 2), '.')
        && !matches!(*ss, "kotlin" | "project" | "buildscript")
    {
        return Some(ss.to_string());
    }
    let ss_context = inside_ss || ids.contains(&"sourceSets");
    if !ss_context {
        return None;
    }
    if let ["val", ss, "by", "getting" | "creating" | "registering"] = ids.as_slice() {
        return Some(ss.to_string());
    }
    if matches!(
        ids.last(),
        Some(&("getByName" | "named" | "maybeCreate" | "create" | "register"))
    ) && let Some(Tok::Str(v, false)) = label.iter().find(|t| matches!(t, Tok::Str(..)))
    {
        return Some(v.clone());
    }
    if parent_is_ss
        && let [ss, "configure"] | [ss] = ids.as_slice()
        && label.len() <= 3
        && !matches!(*ss, "all" | "configureEach" | "matching")
    {
        return Some(ss.to_string());
    }
    None
}

fn parse_script(text: &str, accessors: &BTreeMap<Vec<String>, String>, here: &str) -> ScriptFacts {
    let toks = tokenize(text);
    let mut f = ScriptFacts {
        deps: BTreeSet::new(),
        targets: BTreeSet::new(),
        target_kinds: BTreeMap::new(),
        depends_on: BTreeMap::new(),
        plugins: BTreeSet::new(),
        js_envs: BTreeMap::new(),
    };
    // Block stack: (source set it names, is `sourceSets`, kmp target it configures, is `plugins`)
    let mut blocks: Vec<(Option<String>, bool, Option<String>, bool)> = Vec::new();
    let mut kotlin_depth: Vec<bool> = Vec::new();
    // Call stack of enclosing `name(` callees.
    let mut calls: Vec<Option<String>> = Vec::new();
    let mut stmt_start = 0usize;
    let mut i = 0;
    let resolve = |p: &str| -> String {
        if p.starts_with(':') {
            p.to_string()
        } else if here == ":" {
            format!(":{p}")
        } else {
            format!("{here}:{p}")
        }
    };
    let current_ss = |blocks: &Vec<(Option<String>, bool, Option<String>, bool)>| {
        blocks.iter().rev().find_map(|b| b.0.clone())
    };
    let config_for = |callee: Option<&str>, ss: Option<String>| -> String {
        let c = callee.unwrap_or("?").to_string();
        match ss {
            Some(ss) if !c.starts_with(&ss) => format!("{ss}{}", capitalize(&c)),
            _ => c,
        }
    };
    while i < toks.len() {
        match &toks[i] {
            Tok::Newline | Tok::Punct(';') => stmt_start = i + 1,
            Tok::Punct('{') => {
                let label = &toks[stmt_start..i];
                let inside_ss = blocks.iter().any(|b| b.1);
                let parent_is_ss = blocks.last().map(|b| b.1).unwrap_or(false);
                let ss = block_source_set(label, parent_is_ss, inside_ss);
                let ids: Vec<&str> = label.iter().filter_map(ident).collect();
                let is_ss = ids.last() == Some(&"sourceSets");
                let in_kotlin = kotlin_depth.iter().any(|k| *k);
                let mut target = ids
                    .iter()
                    .rev()
                    .find(|i| KMP_TARGETS.contains(i))
                    .map(|s| s.to_string());
                if let Some(kind) = target.clone()
                    && let Some(at) = label.iter().rposition(|t| ident(t) == Some(kind.as_str()))
                    && let Some(name) = named_target(label, at)
                {
                    f.target_kinds.insert(name.clone(), kind);
                    target = Some(name);
                }
                if in_kotlin && ids.as_slice() == ["android"] {
                    target = Some("androidLibrary".into());
                }
                kotlin_depth.push(ids.last() == Some(&"kotlin"));
                let in_plugins = ids.last() == Some(&"plugins");
                if let Some(t) = &target {
                    f.targets.insert(t.clone());
                }
                if let Some(env) = ids
                    .last()
                    .filter(|i| matches!(**i, "browser" | "nodejs" | "d8"))
                    && let Some(t) = blocks.iter().rev().find_map(|b| b.2.clone())
                {
                    f.js_envs.entry(t).or_default().insert(env.to_string());
                }
                blocks.push((ss, is_ss, target, in_plugins));
                stmt_start = i + 1;
            }
            Tok::Punct('}') => {
                blocks.pop();
                kotlin_depth.pop();
                stmt_start = i + 1;
            }
            Tok::Punct('(') => {
                let callee = if i > 0 {
                    ident(&toks[i - 1]).map(|s| s.to_string())
                } else {
                    None
                };
                if let Some(c) = &callee {
                    if KMP_TARGETS.contains(&c.as_str()) && !is_p(toks.get(i.wrapping_sub(2)), '.')
                    {
                        match named_target(&toks, i - 1) {
                            Some(name) => {
                                f.target_kinds.insert(name.clone(), c.clone());
                                f.targets.insert(name);
                            }
                            None => {
                                f.targets.insert(c.clone());
                            }
                        }
                    }
                    if matches!(c.as_str(), "browser" | "nodejs" | "d8")
                        && let Some(t) = blocks.iter().rev().find_map(|b| b.2.clone())
                    {
                        f.js_envs.entry(t).or_default().insert(c.clone());
                    }
                }
                calls.push(callee);
            }
            Tok::Punct(')') => {
                calls.pop();
            }
            Tok::Ident(id) => {
                let id = id.as_str();
                let in_plugins = blocks.last().map(|b| b.3).unwrap_or(false);
                if in_plugins
                    && matches!(id, "id" | "kotlin" | "alias")
                    && is_p(toks.get(i + 1), '(')
                {
                    let close = close_of(&toks, i + 1);
                    let inner: Vec<String> = toks[i + 2..close.min(toks.len())]
                        .iter()
                        .map(|t| match t {
                            Tok::Ident(s) | Tok::Str(s, _) => s.clone(),
                            Tok::Punct(c) => c.to_string(),
                            Tok::Newline => String::new(),
                        })
                        .collect();
                    let v = inner.concat();
                    f.plugins.insert(if id == "kotlin" {
                        format!("org.jetbrains.kotlin.{v}")
                    } else {
                        v
                    });
                } else if in_plugins
                    && matches!(id, "java" | "application" | "java-library")
                    && !is_p(toks.get(i + 1), '.')
                {
                    f.plugins.insert(id.to_string());
                }
                if id == "project" && !is_p(toks.get(i.wrapping_sub(1)), '.') {
                    // project(":x") / project(path = ":x") / Groovy `project(':x')`, `project path: ':x'`
                    let command_form = matches!(toks.get(i + 1), Some(Tok::Str(..)))
                        || (ident_at(&toks, i + 1) == Some("path") && is_p(toks.get(i + 2), ':'));
                    let args = if is_p(toks.get(i + 1), '(') {
                        (i + 2, close_of(&toks, i + 1))
                    } else if command_form {
                        (i + 1, (i + 4).min(toks.len()))
                    } else {
                        (i + 1, i + 1)
                    };
                    let mut path = None;
                    let mut k = args.0;
                    while k < args.1.min(toks.len()) {
                        if let Tok::Str(v, false) = &toks[k] {
                            let named =
                                k >= 2 && matches!(&toks[k - 2], Tok::Ident(n) if n != "path");
                            let after_eq =
                                k >= 1 && matches!(toks[k - 1], Tok::Punct('=') | Tok::Punct(':'));
                            if !(named && after_eq) {
                                path = Some(v.clone());
                                break;
                            }
                        }
                        k += 1;
                    }
                    if let Some(p) = path {
                        let callee = enclosing_config(&calls, &toks, stmt_start, i);
                        f.deps.insert((
                            config_for(callee.as_deref(), current_ss(&blocks)),
                            resolve(&p),
                        ));
                    }
                } else if id == "projects"
                    && is_p(toks.get(i + 1), '.')
                    && !is_p(toks.get(i.wrapping_sub(1)), '.')
                {
                    let mut chain = Vec::new();
                    let mut k = i + 1;
                    while is_p(toks.get(k), '.') {
                        if let Some(n) = ident_at(&toks, k + 1) {
                            chain.push(n.to_string());
                            k += 2;
                        } else {
                            break;
                        }
                    }
                    for len in (1..=chain.len()).rev() {
                        if let Some(target) = accessors.get(&chain[..len]) {
                            let callee = enclosing_config(&calls, &toks, stmt_start, i);
                            f.deps.insert((
                                config_for(callee.as_deref(), current_ss(&blocks)),
                                target.clone(),
                            ));
                            break;
                        }
                    }
                    i = k;
                    continue;
                } else if id == "dependsOn" && blocks.iter().any(|b| b.1) {
                    // x.dependsOn(y) or dependsOn(y) inside a source set block
                    let src = if is_p(toks.get(i.wrapping_sub(1)), '.') {
                        ident_at(&toks, i.wrapping_sub(2)).map(|s| s.to_string())
                    } else {
                        current_ss(&blocks)
                    };
                    if let (Some(src), true) = (src, is_p(toks.get(i + 1), '(')) {
                        let close = close_of(&toks, i + 1);
                        let arg = toks[i + 2..close.min(toks.len())]
                            .iter()
                            .find_map(|t| match t {
                                Tok::Str(v, false) => Some(v.clone()),
                                Tok::Ident(v)
                                    if !matches!(
                                        v.as_str(),
                                        "getByName" | "named" | "sourceSets" | "get" | "kotlin"
                                    ) =>
                                {
                                    Some(v.clone())
                                }
                                _ => None,
                            });
                        if let Some(a) = arg {
                            f.depends_on.entry(src).or_default().insert(a);
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    f
}

/// The configuration a dependency notation sits in: the nearest enclosing call that is not a
/// notation helper, or (Groovy command syntax) the identifier opening the statement.
fn enclosing_config(
    calls: &[Option<String>],
    toks: &[Tok],
    stmt_start: usize,
    at: usize,
) -> Option<String> {
    const HELPERS: &[&str] = &[
        "project",
        "platform",
        "enforcedPlatform",
        "testFixtures",
        "add",
        "files",
        "listOf",
        "setOf",
    ];
    for c in calls.iter().rev().flatten() {
        if !HELPERS.contains(&c.as_str()) {
            return Some(c.clone());
        }
    }
    // `add("implementation", project(":x"))`
    if calls.iter().rev().flatten().any(|c| c == "add")
        && let Some(Tok::Str(v, _)) = toks[stmt_start..at]
            .iter()
            .find(|t| matches!(t, Tok::Str(..)))
    {
        return Some(v.clone());
    }
    toks[stmt_start..at]
        .iter()
        .find_map(ident)
        .map(|s| s.to_string())
}

// ------------------------------------------------------------------ KMP hierarchy + test tasks

/// Default hierarchy template parent of a source-set *stem* (name without Main/Test).
/// `applyDefaultHierarchyTemplate { group("nonWeb") { withJvm(); withIos() } }` -> ("nonWeb", [jvm, ios]).
fn template_groups(toks: &[Tok]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let Some(t) =
        (0..toks.len()).find(|&k| ident(&toks[k]) == Some("applyDefaultHierarchyTemplate"))
    else {
        return out;
    };
    let Some(open) = (t..toks.len()).find(|&k| is_p(toks.get(k), '{')) else {
        return out;
    };
    let close = close_of(toks, open);
    let mut k = open;
    while k < close {
        if ident(&toks[k]) == Some("group")
            && is_p(toks.get(k + 1), '(')
            && let (Some(Tok::Str(g, false)), Some(b)) = (
                toks.get(k + 2),
                (k..close).find(|&j| is_p(toks.get(j), '{')),
            )
        {
            let end = close_of(toks, b);
            let members = toks[b..end]
                .iter()
                .filter_map(ident)
                .filter_map(|i| i.strip_prefix("with"))
                .filter(|m| !m.is_empty())
                .map(|m| {
                    let m = m[..1].to_lowercase() + &m[1..];
                    // AGP's `withAndroid()` takes the KMP Android library target; `withAndroidTarget()`
                    // stays apart: it takes only `androidTarget()`.
                    if m == "androidLibrary" {
                        "android".to_string()
                    } else {
                        m
                    }
                })
                .collect();
            out.push((g.clone(), members));
        }
        k += 1;
    }
    out
}

fn template_parent(stem: &str) -> Option<&'static str> {
    Some(match stem {
        "jvm" | "android" | "androidHost" | "androidUnit" | "native" | "web" | "wasmWasi" => {
            "common"
        }
        "js" | "wasmJs" => "web",
        "apple" | "linux" | "mingw" | "androidNative" => "native",
        "ios" | "macos" | "tvos" | "watchos" => "apple",
        s if s.starts_with("ios") => "ios",
        s if s.starts_with("macos") => "macos",
        s if s.starts_with("tvos") => "tvos",
        s if s.starts_with("watchos") => "watchos",
        s if s.starts_with("linux") => "linux",
        s if s.starts_with("mingw") => "mingw",
        s if s.starts_with("androidNative") => "androidNative",
        _ => return None,
    })
}

fn split_stem(name: &str) -> Option<(&str, &str)> {
    for suf in ["Main", "Test"] {
        if let Some(stem) = name.strip_suffix(suf)
            && !stem.is_empty()
        {
            return Some((stem, suf));
        }
    }
    None
}

/// Leaf KMP target that a source-set stem belongs to, if it is one.
fn target_of_stem(stem: &str) -> Option<&str> {
    match stem {
        "jvm" | "js" | "wasmJs" | "wasmWasi" => Some(stem),
        "android" | "androidUnit" | "androidHost" => Some("android"),
        s if KMP_TARGETS.contains(&s) => Some(s),
        _ => None,
    }
}

fn leaf_test_tasks(target: &str, project: &ProjectShape) -> Vec<String> {
    match project.kind_of(target) {
        "jvm" => vec![format!("{target}Test")],
        "android" | "androidTarget" | "androidLibrary" => {
            // Only what the build or the layout states: a guessed task name fails the Gradle invocation.
            let unit: Vec<String> = project
                .unit_build_types()
                .into_iter()
                .map(|bt| android_task(UNIT, None, bt))
                .collect();
            if project.android_target_call {
                unit
            } else if project.android_host_test_task() {
                vec!["testAndroidHostTest".into()]
            } else if project.android_unit_dir {
                unit
            } else {
                vec![]
            }
        }
        "js" | "wasmJs" => {
            let envs = project.js_envs.get(target).cloned().unwrap_or_default();
            let mut v: Vec<String> = Vec::new();
            if envs.is_empty() || envs.contains("browser") {
                v.push(format!("{target}BrowserTest"));
            }
            if envs.contains("nodejs") {
                v.push(format!("{target}NodeTest"));
            }
            if envs.contains("d8") {
                v.push(format!("{target}D8Test"));
            }
            v
        }
        "wasmWasi" => vec![format!("{target}NodeTest")],
        // Device-only native targets have no test task.
        "iosArm64" | "tvosArm64" | "watchosArm32" | "watchosArm64" | "watchosDeviceArm64"
        | "androidNativeArm32" | "androidNativeArm64" | "androidNativeX86" | "androidNativeX64"
        | "linuxArm64" => vec![],
        _ => vec![format!("{target}Test")],
    }
}

struct ProjectShape {
    kmp: bool,
    android: bool,
    kmp_android_library: bool,
    android_host_tests: bool,
    android_target_call: bool,
    android_unit_dir: bool,
    js_envs: BTreeMap<String, BTreeSet<String>>,
    /// Named target -> kind (`ScriptFacts::target_kinds`).
    target_kinds: BTreeMap<String, String>,
    /// Android product flavors (one dimension): unit tests run per flavor, `test<Flavor>DebugUnitTest`.
    flavors: Vec<String>,
    /// The build type connected tests run under (`testBuildType`, capitalised): `Debug` unless the
    /// script sets another.
    test_build_type: String,
    /// AGP 9 without `android.onlyEnableUnitTestForTheTestedBuildType=false`: unit tests exist for
    /// the tested build type only. Otherwise (older or unknown AGP) every build type has them.
    tested_only: bool,
}

impl ProjectShape {
    /// Build types whose unit tests run a module's `test` sources: the tested one, plus `Debug`
    /// where every build type has unit tests (the task CI's `test` and `check` run either way).
    fn unit_build_types(&self) -> Vec<&str> {
        let bt = self.test_build_type.as_str();
        if self.tested_only || bt == "Debug" {
            vec![bt]
        } else {
            vec!["Debug", bt]
        }
    }

    /// Do the Android unit tests run under `testAndroidHostTest` (the KMP Android library plugin's
    /// host tests) rather than AGP's `test<Variant>UnitTest`?
    fn android_host_test_task(&self) -> bool {
        !self.android_target_call
            && self.android_host_tests
            && (self.kmp_android_library || !self.android_unit_dir)
    }

    /// Is the Android target the KMP Android library plugin's rather than `androidTarget()`? A
    /// hierarchy template's `withAndroidTarget()` matches only `androidTarget()`'s compilations.
    fn android_library_target(&self) -> bool {
        !self.android_target_call && (self.kmp_android_library || self.android_host_test_task())
    }

    /// The source set Android unit tests compile from: `androidHostTest` or `androidUnitTest`.
    /// Its own `dependsOn` edges decide which shared test source sets run there.
    fn android_test_set(&self) -> &'static str {
        if self.android_host_test_task() {
            "androidHostTest"
        } else {
            "androidUnitTest"
        }
    }

    fn kind_of<'s>(&'s self, target: &'s str) -> &'s str {
        self.target_kinds
            .get(target)
            .map(String::as_str)
            .unwrap_or(target)
    }
}

fn test_tasks_for(
    name: &str,
    sets: &BTreeMap<String, SourceSet>,
    targets: &BTreeSet<String>,
    shape: &ProjectShape,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if !shape.kmp {
        let bt = shape.test_build_type.as_str();
        let flavor_of = |prefix: &str| {
            shape
                .flavors
                .iter()
                .find(|f| name.strip_prefix(prefix) == Some(capitalize(f).as_str()))
        };
        let flavors: Vec<Option<&str>> = if shape.flavors.is_empty() {
            vec![None]
        } else {
            shape.flavors.iter().map(|f| Some(f.as_str())).collect()
        };
        let units = |flavor: Option<&str>| -> Vec<String> {
            shape
                .unit_build_types()
                .into_iter()
                .map(|b| android_task(UNIT, flavor, b))
                .collect()
        };
        if name == "test" && shape.android {
            out.extend(flavors.iter().flat_map(|f| units(*f)));
        } else if name == "test" {
            out.insert("test".into());
        } else if name == "androidTest" {
            out.extend(flavors.iter().map(|f| android_task(CONNECTED, *f, bt)));
        } else if let Some(f) = flavor_of("test").filter(|_| shape.android) {
            out.extend(units(Some(f)));
        } else if let Some(f) = flavor_of("androidTest").filter(|_| shape.android) {
            out.insert(android_task(CONNECTED, Some(f), bt));
        } else if let Some(build_type) = name
            .strip_prefix("test")
            .filter(|r| shape.android && r.starts_with(|c: char| c.is_ascii_uppercase()))
        {
            // A build type's own unit tests (`testRelease`), under each flavor when there are some;
            // none when only the tested build type has unit tests and this is another.
            if !shape.tested_only || build_type == bt {
                out.extend(flavors.iter().map(|f| android_task(UNIT, *f, build_type)));
            }
        } else {
            // A custom JVM test suite (`integrationTest`) gets a task of the same name.
            out.insert(name.to_string());
        }
        return out;
    }
    match name {
        "androidUnitTest" | "androidHostTest" => {
            out.extend(leaf_test_tasks("android", shape));
            return out;
        }
        // Device tests run under managed-device or connected tasks whose names only Gradle knows.
        "androidInstrumentedTest" | "androidDeviceTest" => return out,
        _ => {}
    }
    // Every leaf test source set that (transitively) depends on `name` runs it.
    let mut leaves = BTreeSet::new();
    for t in targets {
        let leaf = if matches!(
            shape.kind_of(t),
            "android" | "androidTarget" | "androidLibrary"
        ) {
            shape.android_test_set().to_string()
        } else {
            format!("{t}Test")
        };
        if depends_transitively(&leaf, name, sets) || leaf == name {
            leaves.insert(t.clone());
        }
    }
    for t in leaves {
        out.extend(leaf_test_tasks(&t, shape));
    }
    out
}

/// AGP 9 (from the version catalog or the root build script) without the opt-out property: AGP 9
/// creates unit tests for the tested build type only, AGP 8 for every build type. Unknown reads as
/// every build type: it selects more, and never misses a unit-test task AGP 8 runs.
fn unit_tests_for_tested_build_type_only(root: &Path, inputs: &mut Vec<String>) -> bool {
    let props = "gradle.properties";
    if let Some(t) = read(root, props) {
        inputs.push(props.into());
        let opted_out = t.lines().any(|l| {
            let l: String = l.chars().filter(|c| !c.is_whitespace()).collect();
            l == "android.onlyEnableUnitTestForTheTestedBuildType=false"
        });
        if opted_out {
            return false;
        }
    }
    agp_major(root, inputs).is_some_and(|m| m >= 9)
}

fn agp_major(root: &Path, inputs: &mut Vec<String>) -> Option<u32> {
    let major = |v: &str| v.split('.').next()?.trim().parse::<u32>().ok();
    let catalog = "gradle/libs.versions.toml";
    if let Some(t) = read(root, catalog) {
        inputs.push(catalog.into());
        if let Ok(doc) = t.parse::<toml::Table>() {
            let versions = doc.get("versions").and_then(|v| v.as_table());
            let version_of = |e: &toml::Value| -> Option<String> {
                match e.get("version")? {
                    toml::Value::String(s) => Some(s.clone()),
                    v => {
                        let r = v.get("ref")?.as_str()?;
                        Some(versions?.get(r)?.as_str()?.to_string())
                    }
                }
            };
            let is_agp = |e: &toml::Value| {
                let s = |k: &str| e.get(k).and_then(|v| v.as_str());
                s("id").is_some_and(|i| i.starts_with("com.android."))
                    || s("module") == Some("com.android.tools.build:gradle")
                    || (s("group") == Some("com.android.tools.build")
                        && s("name") == Some("gradle"))
            };
            let found = ["plugins", "libraries"]
                .iter()
                .filter_map(|k| doc.get(*k).and_then(|t| t.as_table()))
                .flat_map(|t| t.values())
                .filter(|e| is_agp(e))
                .find_map(|e| version_of(e).and_then(|v| major(&v)));
            if found.is_some() {
                return found;
            }
        }
    }
    // `id("com.android.application") version "9.0.0"` in the root build script.
    let toks = tokenize(&read(root, &script_in(root, "", "build")?)?);
    (0..toks.len()).find_map(|i| match (&toks[i], toks.get(i + 2), toks.get(i + 3)) {
        (Tok::Str(id, false), Some(t), Some(Tok::Str(v, false)))
            if id.starts_with("com.android.") && ident(t) == Some("version") =>
        {
            major(v)
        }
        _ => None,
    })
}

const UNIT: (&str, &str) = ("test", "UnitTest");
const CONNECTED: (&str, &str) = ("connected", "AndroidTest");

/// An Android variant's test task: `test<Flavor><BuildType>UnitTest`, `connected<…>AndroidTest`.
fn android_task((prefix, suffix): (&str, &str), flavor: Option<&str>, build_type: &str) -> String {
    let flavor = flavor.map(capitalize).unwrap_or_default();
    format!("{prefix}{flavor}{build_type}{suffix}")
}

/// The build type unit and connected tests run under: `testBuildType = "release"` (Groovy:
/// `testBuildType "release"`), capitalised; `Debug` when unset.
fn test_build_type(toks: &[Tok]) -> String {
    toks.iter()
        .enumerate()
        .filter(|(_, t)| ident(t) == Some("testBuildType"))
        .find_map(|(i, _)| {
            let at = if is_p(toks.get(i + 1), '=') {
                i + 2
            } else {
                i + 1
            };
            match toks.get(at) {
                Some(Tok::Str(b, false)) if !b.is_empty() => Some(capitalize(b)),
                _ => None,
            }
        })
        .unwrap_or_else(|| "Debug".into())
}

/// The flavors a `productFlavors { create("x") }` (or `register`, `maybeCreate`, Groovy `x { }`) block
/// declares. Empty with more than one flavor dimension: each variant then combines flavors in
/// dimension order, which this does not model.
fn product_flavors(toks: &[Tok]) -> Vec<String> {
    let mut out = Vec::new();
    let mut dimensions = BTreeSet::new();
    for i in 0..toks.len() {
        if ident(&toks[i]) == Some("dimension")
            && is_p(toks.get(i + 1), '=')
            && let Some(Tok::Str(d, _)) = toks.get(i + 2)
        {
            dimensions.insert(d.clone());
        }
        if ident(&toks[i]) != Some("productFlavors") || !is_p(toks.get(i + 1), '{') {
            continue;
        }
        let end = close_of(toks, i + 1);
        let mut j = i + 2;
        while j < end {
            let named = match (ident(&toks[j]), toks.get(j + 1), toks.get(j + 2)) {
                (
                    Some("create" | "register" | "maybeCreate"),
                    Some(Tok::Punct('(')),
                    Some(Tok::Str(n, false)),
                ) => Some(n.clone()),
                (Some(n), Some(Tok::Punct('{')), _) => Some(n.to_string()),
                _ => None,
            };
            if let Some(n) = named
                && !out.contains(&n)
            {
                out.push(n);
            }
            // Skip the flavor's own block: its contents are not flavors.
            j = match toks[j + 1..end]
                .iter()
                .position(|t| matches!(t, Tok::Punct('{')))
            {
                Some(k) if named_block_at(toks, j, j + 1 + k) => close_of(toks, j + 1 + k) + 1,
                _ => j + 1,
            };
        }
    }
    if dimensions.len() > 1 { vec![] } else { out }
}

/// Is the `{` at `open` the block of the flavor declaration starting at `at` (same statement)?
fn named_block_at(toks: &[Tok], at: usize, open: usize) -> bool {
    !toks[at..open]
        .iter()
        .any(|t| *t == Tok::Newline || matches!(t, Tok::Punct(';')))
}

fn depends_transitively(from: &str, to: &str, sets: &BTreeMap<String, SourceSet>) -> bool {
    let mut seen = BTreeSet::new();
    let mut q = vec![from.to_string()];
    while let Some(cur) = q.pop() {
        if cur == to {
            return true;
        }
        if !seen.insert(cur.clone()) {
            continue;
        }
        if let Some(ss) = sets.get(&cur) {
            q.extend(ss.depends_on.iter().cloned());
        } else if let Some((stem, suf)) = split_stem(&cur)
            && let Some(p) = template_parent(stem)
        {
            q.push(format!("{p}{suf}"));
        }
    }
    false
}

// ------------------------------------------------------------------ discover

/// `<stem>/src` -> `<stem>Main` and `<stem>/test` -> `<stem>Test` for each child directory of a
/// project named like a source-set stem (`common`, `jvm`, `nonJvm`).
fn flat_source_dirs(project_dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(project_dir) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stem_like = name.starts_with(|c: char| c.is_ascii_lowercase())
            && name.chars().all(|c| c.is_ascii_alphanumeric());
        if !stem_like || name == "src" || name == "build" || !e.path().is_dir() {
            continue;
        }
        for (sub, suf) in [("src", "Main"), ("test", "Test")] {
            if e.path().join(sub).is_dir() {
                out.insert(format!("{name}/{sub}"), format!("{name}{suf}"));
            }
        }
    }
    out
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn script_in(root: &Path, dir: &str, stem: &str) -> Option<String> {
    for ext in [".gradle.kts", ".gradle"] {
        let rel = if dir.is_empty() {
            format!("{stem}{ext}")
        } else {
            format!("{dir}/{stem}{ext}")
        };
        if root.join(&rel).is_file() {
            return Some(rel);
        }
    }
    None
}

/// Static discovery, refined by a current Gradle export when one exists.
pub fn discover_best(root: &Path) -> Build {
    let mut b = discover(root);
    if let Some((exp, files)) = fresh_export(root, &b.inputs) {
        merge_export(&mut b, &exp);
        b.inputs.extend(files);
    } else if !b.included_builds.is_empty() {
        b.warnings.push(
            "included builds may contribute convention plugins that add dependencies static discovery cannot see; \
             run `kt-affected discover --gradle-export` for exact ones"
                .into(),
        );
    }
    b
}

/// Static discovery of the build at `root` and, one level deep, of each build it includes.
pub fn discover(root: &Path) -> Build {
    let mut b = discover_one(root);
    let settings = ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .find_map(|f| std::fs::read_to_string(root.join(f)).ok())
        .unwrap_or_default();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for dir in b.included_builds.clone() {
        if dir.is_empty() || dir == "." || dir.starts_with("..") || !root.join(&dir).is_dir() {
            continue;
        }
        let name = included_build_name(&settings, &dir);
        if !names.insert(name.clone()) || b.projects.iter().any(|p| p.path == format!(":{name}")) {
            b.warnings.push(format!(
                "included build {dir}: its name `{name}` collides with another build or project; its tests are not selected"
            ));
            continue;
        }
        let inner = discover_one(&root.join(&dir));
        let at = |rel: &str| {
            if rel.is_empty() {
                dir.clone()
            } else {
                format!("{dir}/{rel}")
            }
        };
        let rename = |p: &str| {
            if p == ":" {
                format!(":{name}")
            } else {
                format!(":{name}{p}")
            }
        };
        let projects = inner
            .projects
            .into_iter()
            .filter(|p| !p.dir.starts_with(".."))
            .map(|mut p| {
                p.path = rename(&p.path);
                p.dir = at(&p.dir);
                p.build_file = p.build_file.as_deref().map(at);
                p.deps = p.deps.into_iter().map(|(c, t)| (c, rename(&t))).collect();
                p
            })
            .collect();
        b.inputs.extend(
            inner
                .inputs
                .iter()
                .filter(|i| !i.starts_with(".."))
                .map(|i| at(i)),
        );
        b.included.push(Included {
            dir: dir.clone(),
            name,
            projects,
        });
    }
    b.inputs.sort();
    b.inputs.dedup();
    b
}

/// An included build's name: `includeBuild("dir") { name = "x" }`, else the directory's name.
fn included_build_name(settings: &str, dir: &str) -> String {
    let re = regex::Regex::new(&format!(
        r#"includeBuild\s*\(?\s*["'](?:\./)?{}/?["']\s*\)?\s*\{{[^{{}}]*?\bname\s*=\s*["']([^"']+)["']"#,
        regex::escape(dir)
    ))
    .unwrap();
    match re.captures(settings) {
        Some(c) => c[1].to_string(),
        None => dir.rsplit('/').next().unwrap_or(dir).to_string(),
    }
}

fn discover_one(root: &Path) -> Build {
    let mut warnings = Vec::new();
    let settings_file = script_in(root, "", "settings");
    let settings = settings_file
        .as_deref()
        .and_then(|f| read(root, f))
        .map(|t| parse_settings(&t));
    let settings = settings.unwrap_or_else(|| {
        warnings.push("no settings.gradle(.kts): single-project build".into());
        Settings {
            root_name: None,
            includes: vec![],
            project_dirs: BTreeMap::new(),
            included_builds: vec![],
            warnings: vec![],
            dynamic: false,
            literals: BTreeSet::new(),
        }
    });
    let mut settings = settings;
    let mut scanned_dirs = Vec::new();
    if settings.dynamic {
        scanned_dirs = add_scanned_projects(root, &mut settings, &mut warnings);
    }
    warnings.extend(settings.warnings.iter().cloned());
    let root_name = settings.root_name.clone().unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });

    // path -> (dir, implicit)
    let mut paths: BTreeMap<String, (String, bool)> = BTreeMap::new();
    paths.insert(":".into(), (String::new(), false));
    for (p, flat) in &settings.includes {
        let segs: Vec<&str> = p
            .trim_start_matches(':')
            .split(':')
            .filter(|s| !s.is_empty())
            .collect();
        for k in 1..segs.len() {
            let parent = format!(":{}", segs[..k].join(":"));
            paths
                .entry(parent)
                .or_insert_with(|| (segs[..k].join("/"), true));
        }
        let dir = if *flat {
            format!("../{}", segs.join("/"))
        } else {
            segs.join("/")
        };
        paths.insert(p.clone(), (dir, false));
    }
    // projectDir overrides; a child without its own override sits under its parent's dir.
    let keys: Vec<String> = paths.keys().cloned().collect();
    for k in &keys {
        if let Some(d) = settings.project_dirs.get(k) {
            paths.get_mut(k).unwrap().0 = d.clone();
        } else if k != ":" {
            let segs: Vec<&str> = k.trim_start_matches(':').split(':').collect();
            for n in (1..segs.len()).rev() {
                let anc = format!(":{}", segs[..n].join(":"));
                if let Some(d) = settings.project_dirs.get(&anc) {
                    paths.get_mut(k).unwrap().0 = format!("{d}/{}", segs[n..].join("/"));
                    break;
                }
            }
        }
    }

    let mut accessors: BTreeMap<Vec<String>, String> = BTreeMap::new();
    for p in paths.keys() {
        if p == ":" {
            accessors.insert(vec![accessor_name(&root_name)], ":".into());
            continue;
        }
        accessors.insert(
            p.trim_start_matches(':')
                .split(':')
                .map(accessor_name)
                .collect(),
            p.clone(),
        );
    }

    let mut inputs: Vec<String> = settings_file.into_iter().collect();
    // A directory's mtime moves when an entry is added or removed: a new module invalidates discovery.
    inputs.extend(scanned_dirs);
    let conv = crate::conventions::Conventions::load(root, &settings.included_builds);
    inputs.extend(conv.inputs.iter().cloned());
    warnings.extend(conv.warnings.iter().cloned());
    let tested_only = unit_tests_for_tested_build_type_only(root, &mut inputs);
    let mut projects = Vec::new();
    // Warning from a shared plugin -> the projects it applies to.
    let mut conv_warnings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Each project's script and convention plugins are read independently: in parallel.
    let read_project = |path: &str, dir: &str| {
        let build_file = script_in(root, dir, "build");
        let text = build_file
            .as_deref()
            .and_then(|f| read(root, f))
            .unwrap_or_default();
        let facts = parse_script(&text, &accessors, path);
        let fx = conv.effects(
            path,
            &facts.plugins,
            &tokenize(&text),
            build_file.as_deref().unwrap_or(""),
        );
        (build_file, text, facts, fx)
    };
    let mut read_projects: Vec<_> = {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|(path, (dir, _))| {
                (dir.is_empty() || root.join(dir).is_dir()).then(|| read_project(path, dir))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    };
    for (path, (dir, implicit)) in &paths {
        let Some((build_file, text, mut facts, fx)) = read_projects.pop().flatten() else {
            warnings.push(format!("{path}: directory {dir} missing"));
            continue;
        };
        if let Some(b) = &build_file {
            inputs.push(b.clone());
        }
        let mut dep_confidence = BTreeMap::new();
        let host_tests_by_plugin;
        let if_config_exists;
        let custom_tests;
        let test_filters;
        {
            for ((c, t), conf) in fx.deps {
                if c == "?" {
                    warnings.push(format!(
                        "{path}: dependency on {t} through an unresolved configuration"
                    ));
                    continue;
                }
                if facts.deps.insert((c.clone(), t.clone())) {
                    dep_confidence.insert(format!("{c} {t}"), conf.label().to_string());
                }
            }
            facts.plugins.extend(fx.plugins);
            facts.targets.extend(fx.targets);
            facts.target_kinds.extend(fx.target_kinds);
            for (t, e) in fx.js_envs {
                facts.js_envs.entry(t).or_default().extend(e);
            }
            host_tests_by_plugin = fx.android_host_tests;
            if_config_exists = fx.if_config_exists;
            custom_tests = fx.custom_tests;
            test_filters = fx.test_filters;
            for w in fx.warnings {
                conv_warnings.entry(w).or_default().push(path.clone());
            }
        }
        let mut source_sets: BTreeMap<String, SourceSet> = BTreeMap::new();
        let src = if dir.is_empty() {
            root.join("src")
        } else {
            root.join(dir).join("src")
        };
        if let Ok(rd) = std::fs::read_dir(&src) {
            for e in rd.flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let n = e.file_name().to_string_lossy().into_owned();
                    source_sets.insert(
                        n.clone(),
                        SourceSet {
                            test: is_test_source_set(&n),
                            depends_on: BTreeSet::new(),
                        },
                    );
                }
            }
        }
        let plugin_text = facts
            .plugins
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        // A build whose logic re-points source directories (`kotlin.setSrcDirs(..)`) from a
        // template, with no `src/` here: the flattened `<set stem>/src`, `<set stem>/test` layout.
        let mut source_dirs = BTreeMap::new();
        if source_sets.is_empty() && (text.contains("setSrcDirs") || conv.mentions("setSrcDirs")) {
            source_dirs = flat_source_dirs(&root.join(dir));
            for set in source_dirs.values() {
                source_sets.insert(
                    set.clone(),
                    SourceSet {
                        test: is_test_source_set(set),
                        depends_on: BTreeSet::new(),
                    },
                );
            }
        }
        let kmp = plugin_text.contains("multiplatform")
            || plugin_text.contains("kmp")
            || source_sets.contains_key("commonMain")
            || source_sets.contains_key("commonTest")
            || source_sets
                .keys()
                .any(|k| k.starts_with("jvm") || k.starts_with("wasmJs"));
        let android = plugin_text.contains("android")
            || text.contains("android {")
            || text.contains("androidTarget");
        let kmp_android_library = facts.targets.contains("androidLibrary")
            || plugin_text.contains("kotlin.multiplatform.library");
        let android_host_tests = host_tests_by_plugin
            || text.contains("withHostTest")
            || source_sets.contains_key("androidHostTest");
        let kinds = &facts.target_kinds;
        let kind_of =
            |t: &str| -> String { kinds.get(t).cloned().unwrap_or_else(|| t.to_string()) };
        let mut targets = facts.targets.clone();
        for name in source_sets.keys() {
            if let Some((stem, _)) = split_stem(name) {
                if kinds.contains_key(stem) {
                    targets.insert(stem.to_string());
                } else if let Some(t) = target_of_stem(stem) {
                    targets.insert(t.to_string());
                }
            }
        }
        if kmp {
            for (name, ss) in source_sets.iter_mut() {
                if let Some(explicit) = facts.depends_on.get(name) {
                    ss.depends_on.extend(explicit.iter().cloned());
                }
                if ss.depends_on.is_empty()
                    && let Some((stem, suf)) = split_stem(name)
                {
                    if let Some(p) = template_parent(&kind_of(stem)) {
                        ss.depends_on.insert(format!("{p}{suf}"));
                    } else if stem.starts_with("android") {
                        ss.depends_on.insert(format!("common{suf}"));
                    }
                }
                // Explicit dependsOn edges whose source set has no directory still shape the hierarchy.
            }
            for (name, deps) in &facts.depends_on {
                source_sets
                    .entry(name.clone())
                    .or_insert_with(|| SourceSet {
                        test: is_test_source_set(name),
                        depends_on: deps.clone(),
                    });
            }
        }
        let (flavors, test_build_type) = if android {
            let toks = tokenize(&text);
            (product_flavors(&toks), test_build_type(&toks))
        } else {
            (vec![], "Debug".into())
        };
        let shape = ProjectShape {
            kmp,
            android,
            kmp_android_library,
            android_host_tests,
            android_target_call: facts.targets.contains("androidTarget"),
            android_unit_dir: source_sets.contains_key("androidUnitTest"),
            js_envs: facts.js_envs.clone(),
            target_kinds: kinds.clone(),
            flavors,
            test_build_type,
            tested_only,
        };
        // Custom hierarchy-template groups: members gain the group as a parent; members without a
        // directory still route through it, so a virtual hierarchy carries them.
        let mut hier = source_sets.clone();
        // A named target's source sets exist without a directory too, under its kind's parent.
        for (name, kind) in kinds {
            for suf in ["Main", "Test"] {
                hier.entry(format!("{name}{suf}"))
                    .or_insert_with(|| SourceSet {
                        test: suf == "Test",
                        depends_on: template_parent(kind)
                            .map(|p| format!("{p}{suf}"))
                            .into_iter()
                            .collect(),
                    });
            }
        }
        if kmp {
            for (group, mut members) in template_groups(&tokenize(&text)) {
                // `withJvm()` takes every target of that kind, `jvm("desktop")` included.
                let named: Vec<String> = kinds
                    .iter()
                    .filter(|(_, k)| *k != "androidTarget" && members.contains(k))
                    .map(|(n, _)| n.clone())
                    .collect();
                members.extend(named);
                for suf in ["Main", "Test"] {
                    for m in &members {
                        // Each matches one plugin's target, and only its main and unit-test
                        // compilations: device tests are outside the template's source set trees.
                        let child = match (m.as_str(), suf) {
                            ("androidTarget", _) if shape.android_library_target() => continue,
                            ("android", _) if shape.android_target_call => continue,
                            ("androidTarget" | "android", "Main") => "androidMain".to_string(),
                            ("androidTarget" | "android", _) => {
                                shape.android_test_set().to_string()
                            }
                            _ => format!("{m}{suf}"),
                        };
                        let parent = split_stem(&child)
                            .and_then(|(stem, _)| template_parent(&kind_of(stem)))
                            .map(|p| format!("{p}{suf}"));
                        for sets in [&mut source_sets, &mut hier] {
                            if let Some(ss) = sets.get_mut(&child) {
                                ss.depends_on.insert(format!("{group}{suf}"));
                            }
                        }
                        let e = hier.entry(child.clone()).or_insert_with(|| SourceSet {
                            test: false,
                            depends_on: parent.into_iter().collect(),
                        });
                        e.depends_on.insert(format!("{group}{suf}"));
                    }
                }
            }
        }
        let mut test_tasks = BTreeMap::new();
        for (name, ss) in &source_sets {
            if ss.test {
                test_tasks.insert(name.clone(), test_tasks_for(name, &hier, &targets, &shape));
            }
        }
        // A hand-registered test task runs wherever that source set's own tasks run.
        for (ss, task) in &custom_tests {
            let ss = ss
                .clone()
                .unwrap_or_else(|| if kmp { "jvmTest".into() } else { "test".into() });
            let own = test_tasks_for(&ss, &hier, &targets, &shape);
            for (name, tasks) in test_tasks.iter_mut() {
                if *name == ss || tasks.iter().any(|t| own.contains(t)) {
                    tasks.insert(task.clone());
                }
            }
        }
        let known: BTreeSet<&String> = paths.keys().collect();
        let mut deps = BTreeSet::new();
        // A KMP project's configurations are prefixed by a target or `common`; a JVM project's are not.
        let config_exists = |c: &str| {
            let names = || kinds.keys().map(String::as_str);
            let prefixed = KMP_TARGETS
                .iter()
                .copied()
                .chain(["common", "android"])
                .chain(names())
                .any(|t| c.starts_with(t) && c[t.len()..].starts_with(char::is_uppercase));
            let target_ok = KMP_TARGETS
                .iter()
                .copied()
                .chain(["common"])
                .chain(names())
                .any(|t| c.starts_with(t) && (t == "common" || targets.contains(t)));
            if kmp {
                prefixed && target_ok
            } else {
                !prefixed
            }
        };
        for (c, t) in facts.deps {
            if if_config_exists.contains(&(c.clone(), t.clone())) && !config_exists(&c) {
                continue;
            }
            if known.contains(&t) {
                deps.insert((c, t));
            } else {
                warnings.push(format!("{path}: dependency on unknown project {t}"));
            }
        }
        dep_confidence.retain(|k: &String, _| deps.iter().any(|(c, t)| *k == format!("{c} {t}")));
        projects.push(Project {
            path: path.clone(),
            dir: dir.clone(),
            build_file,
            deps,
            source_sets,
            targets,
            test_tasks,
            implicit: *implicit,
            dep_confidence,
            test_filters,
            source_dirs,
        });
    }
    for (w, ps) in conv_warnings {
        warnings.push(match ps.as_slice() {
            [p] => format!("{w} (project {p})"),
            _ => format!("{w} ({} projects, e.g. {})", ps.len(), ps[0]),
        });
    }
    Build {
        root_name,
        projects,
        included_builds: settings.included_builds,
        inputs,
        warnings,
        included: vec![],
    }
}

// ------------------------------------------------------------------ Gradle export

#[cfg(feature = "cli")]
pub const INIT_SCRIPT: &str = include_str!("../gradle/kt-affected-export.init.gradle");

#[derive(Debug, Deserialize)]
struct ExportProject {
    path: String,
    dir: String,
    #[serde(rename = "buildFile")]
    build_file: Option<String>,
    deps: Vec<(String, String)>,
    #[serde(rename = "sourceSets")]
    source_sets: BTreeMap<String, Vec<String>>,
    #[serde(rename = "testTasks")]
    test_tasks: Vec<String>,
}

/// Reads the per-project JSON the init script wrote under `<root>/build/kt-affected/export/`.
pub fn read_export(root: &Path, dir: &Path) -> std::io::Result<Build> {
    let mut projects = Vec::new();
    for e in std::fs::read_dir(dir)?.flatten() {
        let text = std::fs::read_to_string(e.path())?;
        let p: ExportProject = serde_json::from_str(&text)
            .map_err(|err| std::io::Error::other(format!("{}: {err}", e.path().display())))?;
        let source_sets = p
            .source_sets
            .into_iter()
            .map(|(n, d)| {
                (
                    n.clone(),
                    SourceSet {
                        test: is_test_source_set(&n),
                        depends_on: d.into_iter().collect(),
                    },
                )
            })
            .collect();
        let mut tasks = BTreeMap::new();
        tasks.insert("*".to_string(), p.test_tasks.into_iter().collect());
        projects.push(Project {
            path: p.path,
            dir: p.dir,
            build_file: p.build_file,
            deps: p.deps.into_iter().collect(),
            source_sets,
            targets: BTreeSet::new(),
            test_tasks: tasks,
            implicit: false,
            dep_confidence: BTreeMap::new(),
            test_filters: Default::default(),
            source_dirs: BTreeMap::new(),
        });
    }
    projects.sort_by(|a, b| a.path.cmp(&b.path));
    let _ = root;
    Ok(Build {
        root_name: String::new(),
        projects,
        included_builds: vec![],
        inputs: vec![],
        warnings: vec![],
        included: vec![],
    })
}

#[cfg(feature = "cli")]
pub fn write_init_script(to: &Path) -> std::io::Result<()> {
    std::fs::write(to, INIT_SCRIPT)
}

/// Runs `./gradlew ktAffectedExport` with the shipped init script (the project's own build).
#[cfg(feature = "cli")]
pub fn gradle_export(root: &Path) -> std::io::Result<Build> {
    let dir = root.join("build/kt-affected");
    std::fs::create_dir_all(&dir)?;
    let script = dir.join("kt-affected-export.init.gradle");
    write_init_script(&script)?;
    let out = root.join("build/kt-affected/export");
    let _ = std::fs::remove_dir_all(&out);
    let wrapper: std::path::PathBuf = if cfg!(windows) {
        root.join("gradlew.bat")
    } else {
        root.join("gradlew")
    };
    let status = std::process::Command::new(wrapper)
        .current_dir(root)
        .args(["-q", "--init-script"])
        .arg(&script)
        .arg("ktAffectedExport")
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "gradle export failed: {status}"
        )));
    }
    read_export(root, &out)
}

#[cfg(feature = "cli")]
#[derive(Debug, Default, Serialize)]
pub struct Comparison {
    pub projects_static: usize,
    pub projects_other: usize,
    pub projects_matched: usize,
    pub only_static: Vec<String>,
    pub only_other: Vec<String>,
    pub dep_edges_static: usize,
    pub dep_edges_other: usize,
    pub dep_edges_matched: usize,
    pub missing_deps: Vec<String>,
    pub extra_deps: Vec<String>,
    pub source_set_dirs_undeclared: Vec<String>,
    pub test_task_mismatches: Vec<String>,
}

/// Static discovery vs a reference (export or another engine), at project and project-dep level.
#[cfg(feature = "cli")]
pub fn compare(stat: &Build, other: &Build, compare_tasks: bool) -> Comparison {
    let sp: BTreeSet<&str> = stat
        .projects
        .iter()
        .filter(|p| !p.implicit || p.build_file.is_some())
        .map(|p| p.path.as_str())
        .collect();
    let op: BTreeSet<&str> = other.projects.iter().map(|p| p.path.as_str()).collect();
    let mut c = Comparison {
        projects_static: sp.len(),
        projects_other: op.len(),
        projects_matched: sp.intersection(&op).count(),
        only_static: sp.difference(&op).map(|s| s.to_string()).collect(),
        only_other: op.difference(&sp).map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    let edges = |b: &Build| -> BTreeSet<(String, String)> {
        b.projects
            .iter()
            .flat_map(|p| {
                p.deps
                    .iter()
                    .filter(|(_, t)| *t != p.path)
                    .map(move |(_, t)| (p.path.clone(), t.clone()))
            })
            .collect()
    };
    let se = edges(stat);
    let oe = edges(other);
    c.dep_edges_static = se.len();
    c.dep_edges_other = oe.len();
    c.dep_edges_matched = se.intersection(&oe).count();
    c.missing_deps = oe
        .difference(&se)
        .map(|(a, b)| format!("{a} -> {b}"))
        .collect();
    c.extra_deps = se
        .difference(&oe)
        .map(|(a, b)| format!("{a} -> {b}"))
        .collect();
    for p in &stat.projects {
        if let Some(o) = other.project(&p.path) {
            if o.source_sets.is_empty() {
                continue;
            }
            for name in p.source_sets.keys() {
                if !o.source_sets.contains_key(name) && !matches!(name.as_str(), "main" | "test") {
                    c.source_set_dirs_undeclared
                        .push(format!("{}:{name}", p.path));
                }
            }
            if compare_tasks {
                let predicted: BTreeSet<&String> = p.test_tasks.values().flatten().collect();
                let actual: BTreeSet<&String> = o.test_tasks.values().flatten().collect();
                for t in predicted.difference(&actual) {
                    c.test_task_mismatches
                        .push(format!("{}: predicted {t} not a test task", p.path));
                }
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_filter_admits_classes_the_patterns_can_run() {
        let f = TestFilter {
            include: vec![
                "demo.screens.*".into(),
                "demo.one.Cls.method".into(),
                "demo.other.*UiCheck".into(),
            ],
            exclude: vec!["demo.screens.Slow".into(), "demo.screens.Fast.flaky".into()],
        };
        assert!(f.admits("demo.screens.HomeTest"));
        assert!(f.admits("demo.screens.deep.HomeTest"));
        assert!(
            f.admits("demo.one.Cls"),
            "a method pattern admits its class"
        );
        assert!(f.admits("demo.other.LoginUiCheck"));
        assert!(!f.admits("demo.logic.HomeTest"));
        assert!(!f.admits("demo.one.Other"));
        assert!(
            !f.admits("demo.screens.Slow"),
            "an exclude matching the class removes it"
        );
        assert!(
            f.admits("demo.screens.Fast"),
            "excluding one method leaves the class"
        );
        let simple = TestFilter {
            include: vec!["Home*".into()],
            exclude: vec![],
        };
        assert!(
            simple.admits("demo.screens.HomeTest"),
            "upper-case patterns match the simple name"
        );
        assert!(!simple.admits("demo.screens.AwayTest"));
        assert!(TestFilter::default().admits("anything.AtAll"));
        let star = TestFilter {
            include: vec!["*UiCheck".into()],
            exclude: vec![],
        };
        assert!(
            star.admits("demo.HomeTest"),
            "a method named `..UiCheck` may run"
        );
    }

    use super::*;

    #[test]
    fn test_build_type_reads_kotlin_and_groovy_forms() {
        let bt = |t: &str| test_build_type(&tokenize(t));
        assert_eq!(bt(r#"android { testBuildType = "release" }"#), "Release");
        assert_eq!(bt("android {\n  testBuildType 'staging'\n}"), "Staging");
        assert_eq!(bt(r#"android { testBuildType = "" }"#), "Debug");
        assert_eq!(bt("// testBuildType = \"release\"\nandroid { }"), "Debug");
        assert_eq!(bt("android { }"), "Debug");
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("kt-affected-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn deps_of(b: &Build, p: &str) -> BTreeSet<(String, String)> {
        b.project(p).unwrap().deps.clone()
    }

    #[test]
    fn multi_module_jvm_kotlin_dsl() {
        let r = tmp("jvm");
        write(
            &r,
            "settings.gradle.kts",
            "rootProject.name = \"demo\"\ninclude(\":core\", \":app\")\n// include(\":ghost\")\n",
        );
        write(&r, "core/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
        write(
            &r,
            "app/build.gradle.kts",
            "plugins { kotlin(\"jvm\") }\ndependencies {\n  implementation(project(\":core\"))\n  testImplementation(project(path = \":core\"))\n}\n",
        );
        write(&r, "core/src/main/kotlin/A.kt", "");
        write(&r, "app/src/test/kotlin/T.kt", "");
        let b = discover(&r);
        assert_eq!(b.root_name, "demo");
        assert!(b.project(":ghost").is_none());
        assert_eq!(
            deps_of(&b, ":app"),
            BTreeSet::from([
                ("implementation".into(), ":core".into()),
                ("testImplementation".into(), ":core".into())
            ])
        );
        assert_eq!(
            b.project(":app").unwrap().test_tasks["test"],
            BTreeSet::from(["test".to_string()])
        );
    }

    #[test]
    fn groovy_settings_and_command_syntax() {
        let r = tmp("groovy");
        write(
            &r,
            "settings.gradle",
            "include ':lib',\n  ':tool'\nproject(':tool').projectDir = new File(settingsDir, 'tools/tool')\n",
        );
        write(&r, "lib/build.gradle", "apply plugin: 'java'\n");
        write(
            &r,
            "tools/tool/build.gradle",
            "dependencies {\n  implementation project(':lib')\n}\n",
        );
        let b = discover(&r);
        assert_eq!(b.project(":tool").unwrap().dir, "tools/tool");
        assert_eq!(
            deps_of(&b, ":tool"),
            BTreeSet::from([("implementation".into(), ":lib".into())])
        );
    }

    #[test]
    fn named_kmp_targets_get_their_own_source_sets_and_test_tasks() {
        let r = tmp("named-targets");
        write(&r, "settings.gradle.kts", "include(\":ui\")\n");
        write(
            &r,
            "ui/build.gradle.kts",
            "plugins { kotlin(\"multiplatform\") }\nkotlin {\n  jvm(\"desktop\")\n  js(\"site\") { browser() }\n  wasmJs(\"wasmApp\") { nodejs() }\n  iosSimulatorArm64(\"sim\")\n  iosArm64(\"device\")\n  sourceSets {\n    val desktopTest by getting { dependencies { implementation(kotlin(\"test\")) } }\n  }\n}\n",
        );
        for d in [
            "commonTest",
            "desktopMain",
            "desktopTest",
            "simTest",
            "siteTest",
        ] {
            write(&r, &format!("ui/src/{d}/kotlin/X.kt"), "");
        }
        let b = discover(&r);
        let ui = b.project(":ui").unwrap();
        let set = |v: &[&str]| {
            v.iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<String>>()
        };
        assert_eq!(
            ui.test_tasks["commonTest"],
            set(&[
                "desktopTest",
                "siteBrowserTest",
                "wasmAppNodeTest",
                "simTest"
            ])
        );
        assert_eq!(ui.test_tasks["desktopTest"], set(&["desktopTest"]));
        assert_eq!(ui.test_tasks["simTest"], set(&["simTest"]));
        assert_eq!(ui.test_tasks["siteTest"], set(&["siteBrowserTest"]));
        assert_eq!(
            ui.source_sets["desktopMain"].depends_on,
            set(&["commonMain"])
        );
        assert_eq!(ui.source_sets["simTest"].depends_on, set(&["iosTest"]));
        assert!(!ui.targets.contains("jvm"), "{:?}", ui.targets);
        assert!(
            ui.deps.is_empty() && !b.warnings.iter().any(|w| w.contains("desktopTest")),
            "{:?}",
            b.warnings
        );
    }

    #[test]
    fn kmp_typesafe_accessors_and_source_sets() {
        let r = tmp("kmp");
        write(
            &r,
            "settings.gradle.kts",
            "enableFeaturePreview(\"TYPESAFE_PROJECT_ACCESSORS\")\ninclude(\":shared:core-api\", \":shared:ui\")\n",
        );
        write(
            &r,
            "shared/core-api/build.gradle.kts",
            "plugins { kotlin(\"multiplatform\") }\nkotlin { jvm() }\n",
        );
        write(
            &r,
            "shared/ui/build.gradle.kts",
            "plugins { kotlin(\"multiplatform\") }\nkotlin {\n  jvm()\n  wasmJs { browser() }\n  iosSimulatorArm64()\n  sourceSets {\n    commonMain.dependencies { implementation(projects.shared.coreApi) }\n    val jvmTest by getting { dependencies { implementation(project(\":shared:core-api\")) } }\n  }\n}\n",
        );
        for d in ["commonMain", "commonTest", "jvmTest", "wasmJsTest"] {
            write(&r, &format!("shared/ui/src/{d}/kotlin/X.kt"), "");
        }
        let b = discover(&r);
        let ui = b.project(":shared:ui").unwrap();
        assert_eq!(
            ui.deps,
            BTreeSet::from([
                ("commonMainImplementation".into(), ":shared:core-api".into()),
                ("jvmTestImplementation".into(), ":shared:core-api".into())
            ])
        );
        assert!(b.project(":shared").unwrap().implicit);
        assert_eq!(
            ui.test_tasks["commonTest"],
            BTreeSet::from([
                "jvmTest".to_string(),
                "wasmJsBrowserTest".to_string(),
                "iosSimulatorArm64Test".to_string()
            ])
        );
        assert_eq!(
            ui.test_tasks["wasmJsTest"],
            BTreeSet::from(["wasmJsBrowserTest".to_string()])
        );
        assert_eq!(
            ui.source_sets["jvmTest"].depends_on,
            BTreeSet::from(["commonTest".to_string()])
        );
    }

    #[test]
    fn plugin_management_include_builds_are_plugin_builds() {
        let s = "pluginManagement {\n  includeBuild(\"build-logic\")\n  repositories { gradlePluginPortal() }\n}\nincludeBuild(\"./shared-lib\")\n";
        let got: Vec<String> = plugin_management_builds(s).into_iter().collect();
        assert_eq!(vec!["build-logic".to_string()], got);
    }

    #[test]
    fn project_dir_override_moves_children_and_include_build_is_separate() {
        let r = tmp("dirs");
        write(
            &r,
            "settings.gradle.kts",
            "pluginManagement { includeBuild(\"build-logic\") }\ninclude(\":feature:a\")\nproject(\":feature\").projectDir = file(\"$rootDir/modules/feature\")\n",
        );
        write(&r, "modules/feature/a/build.gradle.kts", "");
        write(&r, "build-logic/settings.gradle.kts", "");
        let b = discover(&r);
        assert_eq!(b.project(":feature:a").unwrap().dir, "modules/feature/a");
        assert_eq!(b.included_builds, vec!["build-logic".to_string()]);
    }

    #[test]
    fn accessor_names_follow_gradle_camel_case() {
        assert_eq!(accessor_name("core-api"), "coreApi");
        assert_eq!(accessor_name("my_lib"), "myLib");
        assert_eq!(accessor_name("plain"), "plain");
    }

    #[test]
    fn explicit_depends_on_overrides_template() {
        let r = tmp("dependson");
        write(&r, "settings.gradle.kts", "include(\":m\")\n");
        write(
            &r,
            "m/build.gradle.kts",
            "plugins { kotlin(\"multiplatform\") }\nkotlin {\n  jvm()\n  wasmJs()\n  sourceSets {\n    val nonWebTest by creating { dependsOn(commonTest.get()) }\n    jvmTest { dependsOn(nonWebTest) }\n  }\n}\n",
        );
        for d in ["commonTest", "jvmTest", "nonWebTest", "wasmJsTest"] {
            write(&r, &format!("m/src/{d}/kotlin/X.kt"), "");
        }
        let b = discover(&r);
        let m = b.project(":m").unwrap();
        assert_eq!(
            m.source_sets["jvmTest"].depends_on,
            BTreeSet::from(["nonWebTest".to_string()])
        );
        assert_eq!(
            m.test_tasks["nonWebTest"],
            BTreeSet::from(["jvmTest".to_string()])
        );
    }
}

//! What a changed path the declaration graph cannot index reaches: build logic, resources, SQL,
//! SQLDelight, deleted Kotlin, and files outside every module.
//!
//! Cheapest answer first: a file read by name seeds the declarations on lines that write the name
//! (a mention); a migration seeds Kotlin naming its tables; build logic costs one smoke class per
//! test task in reach; code wiring no name reaches fails its module open.

use crate::config::Config;
use crate::git::Hunk;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

#[derive(Clone, Debug)]
pub struct Module {
    pub id: String,
    /// `dir/` (never empty: the root project is not a module here).
    pub prefix: String,
    /// The project's `source_dirs` (`dir relative to prefix` -> source set).
    pub source_dirs: BTreeMap<String, String>,
    /// The project's build script, relative to the root.
    pub build_file: Option<String>,
}

pub struct Modules {
    /// Longest prefix first, so a nested module wins.
    pub list: Vec<Module>,
    pub deps: BTreeMap<String, BTreeSet<String>>,
    pub db: BTreeSet<String>,
}

impl Modules {
    pub fn module_of(&self, path: &str) -> Option<&Module> {
        self.list.iter().find(|m| path.starts_with(&m.prefix))
    }

    /// (module, source set) for `<module>/src/<set>/...` or a file under one of its `source_dirs`.
    pub fn source_set_of<'s>(&'s self, path: &'s str) -> Option<(&'s Module, &'s str)> {
        let m = self.module_of(path)?;
        let rest = &path[m.prefix.len()..];
        crate::discover::source_set_at(&m.source_dirs, rest).map(|s| (m, s))
    }

    pub fn reverse_closure(&self, targets: &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = targets.clone();
        loop {
            let before = out.len();
            for (m, refs) in &self.deps {
                if !out.contains(m) && refs.iter().any(|r| out.contains(r)) {
                    out.insert(m.clone());
                }
            }
            if out.len() == before {
                return out;
            }
        }
    }

    pub fn closure(&self, targets: &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = targets.clone();
        let mut stack: Vec<String> = targets.iter().cloned().collect();
        while let Some(m) = stack.pop() {
            for d in self.deps.get(&m).into_iter().flatten() {
                if out.insert(d.clone()) {
                    stack.push(d.clone());
                }
            }
        }
        out
    }

    pub fn all(&self) -> BTreeSet<String> {
        self.list.iter().map(|m| m.id.clone()).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mention {
    pub tokens: Vec<String>,
    pub modules: BTreeSet<String>,
    pub string_only: bool,
    /// A deleted file's names count only where its package is visible.
    pub package: Option<String>,
    /// A file must contain one of these to count (empty: every file).
    pub file_markers: Vec<String>,
}

#[derive(Default)]
pub struct Routing {
    /// Modules every test class of which runs: build logic changed how their tests execute or
    /// what their code compiles to.
    pub full_modules: BTreeSet<String>,
    pub smoke_modules: BTreeSet<String>,
    pub db_smoke_modules: BTreeSet<String>,
    pub mentions: Vec<Mention>,
    pub reasons: Vec<String>,
}

pub struct Ctx<'a> {
    pub read: &'a dyn Fn(&str) -> Option<String>,
    pub read_base: &'a dyn Fn(&str) -> Option<String>,
    /// Every build script's and build-logic source's text.
    pub build_logic: &'a dyn Fn() -> &'a [String],
    /// Inside an included build that supplies plugins (see `select::included_builds`).
    pub in_plugin_build: &'a dyn Fn(&str) -> bool,
}

const BUILD_LOGIC_NAMES: &[&str] = &[
    "settings.gradle.kts",
    "settings.gradle",
    "gradle.properties",
    "local.properties",
];
const BUILD_LOGIC_PREFIXES: &[&str] = &["gradle/", "buildSrc/"];
const BUILD_CONVENTION_DIRS: &[&str] = &["karma.config.d/", "webpack.config.d/"];
const BUILD_CONVENTION_NAMES: &[&str] = &[
    "AndroidManifest.xml",
    "proguard-rules.pro",
    "consumer-rules.pro",
];
const RESOURCE_ROOTS: &[&str] = &["resources", "composeResources", "res"];
/// Reached through a generated accessor (`Res.drawable.x`, `R.drawable.x`): the type directory
/// names every resource of the type, not this one.
const ACCESSOR_RESOURCE_ROOTS: &[&str] = &["composeResources", "res"];

static RUNTIME_CONFIG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(^|/)(logback[^/]*\.xml|log4j2?[^/]*|junit-platform\.properties|[^/]*\.properties)$",
    )
    .unwrap()
});
static WIRING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|/)(META-INF/|values[^/]*/)|\.conf$").unwrap());
static SQL_TARGET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:TABLE|INTO|UPDATE|FROM|JOIN|ON|VIEW|TYPE|TRUNCATE|SEQUENCE)\s+(?:ONLY\s+|IF\s+(?:NOT\s+)?EXISTS\s+)*(?:"?\w+"?\.)?"?([A-Za-z_]\w*)"?"#).unwrap()
});
static SQL_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)--[^\n]*|/\*.*?\*/").unwrap());
static SQ_TABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bCREATE\s+(?:TABLE|VIEW)\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_]\w*)").unwrap()
});
static SQ_LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^([A-Za-z_]\w*)\s*:").unwrap());
const SQL_WORDS: &[&str] = &[
    "select",
    "set",
    "where",
    "only",
    "exists",
    "not",
    "null",
    "table",
    "values",
    "default",
    "conflict",
    "delete",
    "insert",
    "update",
    "nothing",
    "do",
    "cascade",
    "restrict",
    "each",
    "row",
    "statement",
    "execute",
    "function",
    "procedure",
    "lateral",
    "public",
];

fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn stem(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

pub fn is_build_logic(path: &str) -> bool {
    path.ends_with(".gradle.kts")
        || path.ends_with(".gradle")
        || BUILD_LOGIC_NAMES.contains(&name_of(path))
        || BUILD_LOGIC_PREFIXES.iter().any(|p| path.starts_with(p))
}

fn names_build_logic(name: &str, ctx: &Ctx) -> bool {
    (ctx.build_logic)().iter().any(|t| t.contains(name))
}

fn resource_tokens(rel: &str, root: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let name = parts[parts.len() - 1];
    let mut out: BTreeSet<String> = [name.to_string(), rel.to_string()].into_iter().collect();
    if stem(name).len() >= 3 {
        out.insert(stem(name).to_string());
    }
    if !ACCESSOR_RESOURCE_ROOTS.contains(&root) {
        out.extend(
            parts[..parts.len() - 1]
                .iter()
                .filter(|p| p.len() >= 3)
                .map(|p| p.to_string()),
        );
    }
    out.into_iter().collect()
}

/// Test-task settings that change how every test of a task executes (its JVM, framework,
/// forking), not whether a script still configures: a smoke class cannot observe them.
static TEST_JVM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:(\w+)\s*\.\s*)?\b(jvmArgs|jvmArgumentProviders|allJvmArgs|maxHeapSize|minHeapSize|systemPropert(?:y|ies)|environment|forkEvery|maxParallelForks|jvmToolchain|javaLauncher|toolchain|jvmTarget|useJUnitPlatform|useJUnit|useTestNG|testFramework)\b",
    )
    .unwrap()
});

/// Lines of `path` added or removed between base and head, comments dropped.
fn changed_lines(path: &str, ctx: &Ctx) -> Vec<String> {
    let head = (ctx.read)(path).unwrap_or_default();
    let base = (ctx.read_base)(path).unwrap_or_default();
    let count = |t: &str| {
        let mut m: BTreeMap<String, i64> = BTreeMap::new();
        for l in t.lines() {
            let l = l.trim();
            if !(l.is_empty()
                || l.starts_with("//")
                || l.starts_with('*')
                || l.starts_with("/*")
                || l.starts_with('#'))
            {
                *m.entry(l.to_string()).or_default() += 1;
            }
        }
        m
    };
    let (h, b) = (count(&head), count(&base));
    let mut out = Vec::new();
    for l in h.keys().chain(b.keys()) {
        if h.get(l) != b.get(l) {
            out.push(l.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The test-JVM setting a build-logic edit changes (`jvmArgs`, `maxParallelForks`, ..), if any. A
/// build logic's own tests (`src/test/`) configure nothing.
fn test_jvm_setting(path: &str, ctx: &Ctx) -> Option<String> {
    if path.contains("/src/test/") || path.ends_with(".toml") {
        return None;
    }
    changed_lines(path, ctx).iter().find_map(|l| {
        TEST_JVM_RE.captures_iter(l).find_map(|c| {
            let recv = c.get(1).map(|m| m.as_str());
            // `providers.systemProperty(..)` / `System.getenv` read the build's own environment.
            (!matches!(recv, Some("providers" | "System" | "gradle"))).then(|| c[2].to_string())
        })
    })
}

/// A version-catalog edit that changes the version of a compiler plugin or code generator
/// (`codegen_plugins`): a changed `[versions]` key some codegen plugin or library takes its version
/// from, or a codegen entry whose own text changed.
fn codegen_version_change(path: &str, cfg: &Config, ctx: &Ctx) -> Option<String> {
    if !path.ends_with(".versions.toml") {
        return None;
    }
    let head = (ctx.read)(path).unwrap_or_default();
    let base = (ctx.read_base)(path).unwrap_or_default();
    static ENTRY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"^\s*([A-Za-z0-9_.-]+)\s*=\s*(.*)$"#).unwrap());
    static ID_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\b(?:id|module|group)\s*=\s*"([^"]+)"|^\s*"([^":]+)(?::[^"]*)?""#).unwrap()
    });
    static REF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"version\.ref\s*=\s*"([^"]+)""#).unwrap());
    // (table, key) -> value text
    let entries = |t: &str| {
        let mut out: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut table = String::new();
        for l in t.lines() {
            let l = l.split(" #").next().unwrap_or(l);
            let s = l.trim();
            if s.starts_with('[') {
                table = s.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            } else if let Some(c) = ENTRY_RE.captures(s)
                && !s.starts_with('#')
            {
                out.insert((table.clone(), c[1].to_string()), c[2].trim().to_string());
            }
        }
        out
    };
    let (h, b) = (entries(&head), entries(&base));
    let is_codegen = |v: &str| {
        ID_RE.captures(v).is_some_and(|c| {
            let id = c.get(1).or(c.get(2)).map(|m| m.as_str()).unwrap_or("");
            cfg.selector.codegen_plugins.iter().any(|p| {
                id == p || id.starts_with(&format!("{p}.")) || id.starts_with(&format!("{p}:"))
            })
        })
    };
    let changed = |k: &(String, String)| h.get(k) != b.get(k);
    for side in [&h, &b] {
        for (k, v) in side {
            if !matches!(k.0.as_str(), "plugins" | "libraries") || !is_codegen(v) {
                continue;
            }
            if changed(k) {
                return Some(format!("{} `{}`", k.0, k.1));
            }
            if let Some(r) = REF_RE.captures(v) {
                let key = ("versions".to_string(), r[1].to_string());
                if changed(&key) {
                    return Some(format!("version `{}` of {} `{}`", &r[1], k.0, k.1));
                }
            }
        }
    }
    None
}

/// Build logic that changes how tests run or what code compiles to: every class of `scope`'s test
/// tasks runs, beside the smoke classes the change gets as build logic.
fn whole_task_effect(
    path: &str,
    scope: &BTreeSet<String>,
    cfg: &Config,
    ctx: &Ctx,
    out: &mut Routing,
) {
    let why = if let Some(s) = test_jvm_setting(path, ctx) {
        format!("changes the test JVM or framework (`{s}`)")
    } else if let Some(s) = codegen_version_change(path, cfg, ctx) {
        format!("changes a compiler plugin or code generator ({s})")
    } else {
        return;
    };
    out.full_modules.extend(scope.iter().cloned());
    out.reasons.push(format!(
        "{path}: {why} — every class of the test tasks it configures runs"
    ));
}

/// Type-safe accessors (`libs.androidx.core`, `libs.plugins.x`, `libs.bundles.y`) of the catalog
/// entries whose version a `*.versions.toml` edit changes: an entry whose own text changed, one
/// whose `version.ref` key changed, and bundles holding a changed library.
fn changed_catalog_accessors(path: &str, ctx: &Ctx) -> Vec<String> {
    let Some(catalog) = name_of(path).strip_suffix(".versions.toml") else {
        return Vec::new();
    };
    let parse = |t: Option<String>| -> toml::Table {
        t.and_then(|t| t.parse::<toml::Table>().ok())
            .unwrap_or_default()
    };
    let (head, base) = (parse((ctx.read)(path)), parse((ctx.read_base)(path)));
    let table = |t: &toml::Table, name: &str| -> toml::Table {
        t.get(name)
            .and_then(|v| v.as_table())
            .cloned()
            .unwrap_or_default()
    };
    let version_changed =
        |key: &str| table(&head, "versions").get(key) != table(&base, "versions").get(key);
    let changed_in = |name: &str| -> BTreeSet<String> {
        let (h, b) = (table(&head, name), table(&base, name));
        h.keys()
            .chain(b.keys())
            .filter(|k| {
                let (hv, bv) = (h.get(*k), b.get(*k));
                hv != bv
                    || hv
                        .and_then(|v| v.get("version"))
                        .and_then(|v| v.get("ref"))
                        .and_then(|r| r.as_str())
                        .is_some_and(version_changed)
            })
            .cloned()
            .collect()
    };
    let accessor = |kind: &str, key: &str| {
        let key: String = key
            .chars()
            .map(|c| if c == '-' || c == '_' { '.' } else { c })
            .collect();
        if kind.is_empty() {
            format!("{catalog}.{key}")
        } else {
            format!("{catalog}.{kind}.{key}")
        }
    };
    let libraries = changed_in("libraries");
    let mut out: Vec<String> = libraries.iter().map(|k| accessor("", k)).collect();
    out.extend(changed_in("plugins").iter().map(|k| accessor("plugins", k)));
    for (name, members) in table(&head, "bundles") {
        let holds = members.as_array().is_some_and(|m| {
            m.iter()
                .any(|x| x.as_str().is_some_and(|x| libraries.contains(x)))
        });
        if holds {
            out.push(accessor("bundles", &name));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Does `text` write `accessor` as a whole accessor (not a prefix of a longer one)?
fn writes_accessor(text: &str, accessor: &str) -> bool {
    text.match_indices(accessor).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + accessor.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

/// A catalog edit changing the version of a library or plugin modules declare by accessor: those
/// modules and their dependents run every class of their test tasks, since the new version is on
/// their test runtime classpath. Declarations made through a convention plugin are not seen here;
/// those modules keep the build-logic smoke class.
fn catalog_users(path: &str, mods: &Modules, ctx: &Ctx, out: &mut Routing) {
    let accessors = changed_catalog_accessors(path, ctx);
    if accessors.is_empty() {
        return;
    }
    let mut declarers: BTreeSet<String> = BTreeSet::new();
    let mut named: BTreeSet<&str> = BTreeSet::new();
    for m in &mods.list {
        let Some(text) = m.build_file.as_deref().and_then(|b| (ctx.read)(b)) else {
            continue;
        };
        let code: String = text
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        for a in &accessors {
            if writes_accessor(&code, a) {
                declarers.insert(m.id.clone());
                named.insert(a);
            }
        }
    }
    if declarers.is_empty() {
        return;
    }
    let scope = mods.reverse_closure(&declarers);
    out.reasons.push(format!(
        "{path}: changes the version of {} — declared by {}; every class of their and their dependents' test tasks runs",
        named.into_iter().collect::<Vec<_>>().join(", "),
        declarers.iter().cloned().collect::<Vec<_>>().join(", ")
    ));
    out.full_modules.extend(scope);
}

fn build_logic(path: &str, scope: BTreeSet<String>, out: &mut Routing, why: &str) -> bool {
    out.smoke_modules.extend(scope);
    out.reasons.push(format!(
        "{path}: {why} — one smoke class per test task in reach; CI runs every suite"
    ));
    true
}

/// Route one changed non-graph path into `out`; false leaves it to the caller's fail-open.
pub fn route(path: &str, mods: &Modules, cfg: &Config, ctx: &Ctx, out: &mut Routing) -> bool {
    let module = mods.module_of(path);
    let everything = mods.all();
    let name = name_of(path);
    let Some(module) = module else {
        if is_build_logic(path) || (ctx.in_plugin_build)(path) {
            whole_task_effect(path, &everything, cfg, ctx, out);
            if path.ends_with(".versions.toml") {
                catalog_users(path, mods, ctx, out);
            }
            return build_logic(path, everything, out, "build logic");
        }
        out.mentions.push(Mention {
            tokens: vec![name.into(), path.into()],
            modules: everything.clone(),
            ..Default::default()
        });
        if names_build_logic(name, ctx) {
            return build_logic(path, everything, out, "a build input");
        }
        out.reasons.push(format!(
            "{path}: outside every module — seeding what names it"
        ));
        return true;
    };
    let reach = mods.reverse_closure(&[module.id.clone()].into_iter().collect());
    if path.ends_with(".gradle.kts") || path.ends_with(".gradle") {
        // A module's own script configures its own test tasks only.
        whole_task_effect(
            path,
            &[module.id.clone()].into_iter().collect(),
            cfg,
            ctx,
            out,
        );
        return build_logic(path, reach, out, "build logic");
    }
    if path.ends_with(".sql")
        && cfg
            .database
            .migration_dirs
            .iter()
            .any(|d| path.starts_with(d.as_str()))
    {
        let mut db: BTreeSet<String> = mods.db.clone();
        db.insert(module.id.clone());
        return migration(path, db, mods, cfg, ctx, out);
    }
    if cfg
        .database
        .synced_inputs
        .iter()
        .any(|s| path.starts_with(s.as_str()))
    {
        let scope: BTreeSet<String> = reach.union(&mods.db).cloned().collect();
        return build_logic(path, scope, out, "test-runtime config");
    }
    if path.ends_with(".sq") {
        return sqldelight(path, reach, ctx, out);
    }
    if path.ends_with(".sqm") {
        out.mentions.push(Mention {
            tokens: vec!["Schema.migrate".into()],
            modules: reach,
            ..Default::default()
        });
        out.smoke_modules.insert(module.id.clone());
        out.reasons.push(format!(
            "{path}: SQLDelight migration — its migration tests plus one smoke class"
        ));
        return true;
    }
    if path.ends_with(".kt") {
        return deleted_kotlin(path, reach, ctx, out);
    }
    let Some((_, source_set)) = mods.source_set_of(path) else {
        let rel = &path[module.prefix.len()..];
        out.mentions.push(Mention {
            tokens: vec![name.into(), rel.into()],
            modules: reach.clone(),
            ..Default::default()
        });
        if BUILD_CONVENTION_DIRS.iter().any(|d| rel.starts_with(d))
            || BUILD_CONVENTION_NAMES.contains(&name)
            || names_build_logic(name, ctx)
        {
            return build_logic(path, reach, out, "a build input");
        }
        out.reasons.push(format!(
            "{path}: outside the module's sources — seeding what names it"
        ));
        return true;
    };
    if BUILD_CONVENTION_NAMES.contains(&name) {
        return build_logic(path, reach, out, "a build input");
    }
    let parts: Vec<&str> = path[module.prefix.len()..].split('/').collect(); // src/<set>/<root>/<rel...>
    if parts.len() < 4 || !RESOURCE_ROOTS.contains(&parts[2]) {
        return false;
    }
    let rel = parts[3..].join("/");
    if WIRING_RE.is_match(&rel) {
        return false;
    }
    let visible = if source_set.ends_with("Test") || source_set.ends_with("test") {
        [module.id.clone()].into_iter().collect()
    } else {
        reach
    };
    if RUNTIME_CONFIG_RE.is_match(&rel) {
        return build_logic(path, visible, out, "test-runtime config");
    }
    let mut tokens = resource_tokens(&rel, parts[2]);
    if parts[2] == "resources" {
        // A plain resource is loaded by a string naming it; its bare stem in code (`index`,
        // `config`) is an ordinary identifier, not a read of this file.
        let st = stem(name_of(&rel)).to_string();
        tokens.retain(|t| *t != st);
        if st.len() >= 3 {
            out.mentions.push(Mention {
                tokens: vec![st],
                modules: visible.clone(),
                string_only: true,
                ..Default::default()
            });
        }
    }
    out.mentions.push(Mention {
        tokens,
        modules: visible,
        ..Default::default()
    });
    out.reasons
        .push(format!("{path}: resource — seeding what names it"));
    true
}

/// An ignored or doc path a test may still read by name: seed what names it, in strings only.
/// Nothing naming it selects nothing — the same answer as ignoring it.
pub fn named_only(path: &str, mods: &Modules, out: &mut Routing) {
    let name = name_of(path);
    let mut tokens = vec![path.to_string()];
    if name.len() >= 6 {
        tokens.push(name.to_string());
    }
    out.mentions.push(Mention {
        tokens,
        modules: mods.all(),
        string_only: true,
        ..Default::default()
    });
}

pub fn migration_tables(sql: &str) -> BTreeSet<String> {
    let body = SQL_COMMENT_RE.replace_all(sql, " ");
    SQL_TARGET_RE
        .captures_iter(&body)
        .map(|c| c[1].to_string())
        .filter(|t| !SQL_WORDS.contains(&t.to_lowercase().as_str()) && t.len() >= 3)
        .collect()
}

fn migration(
    path: &str,
    db: BTreeSet<String>,
    mods: &Modules,
    cfg: &Config,
    ctx: &Ctx,
    out: &mut Routing,
) -> bool {
    let sql = (ctx.read)(path)
        .or_else(|| (ctx.read_base)(path))
        .unwrap_or_default();
    let tables = migration_tables(&sql);
    if tables.is_empty() {
        return false;
    }
    let tokens: Vec<String> = tables.iter().cloned().collect();
    out.mentions.push(Mention {
        tokens: tokens.clone(),
        modules: db.clone(),
        string_only: true,
        ..Default::default()
    });
    let upstream: BTreeSet<String> = mods.closure(&db).difference(&db).cloned().collect();
    out.mentions.push(Mention {
        tokens,
        modules: upstream,
        string_only: true,
        file_markers: cfg.database.sql_file_markers.clone(),
        ..Default::default()
    });
    out.db_smoke_modules.extend(db);
    out.reasons.push(format!(
        "{path}: migration — seeding Kotlin that names {}, plus one database-backed class per module",
        tables.iter().cloned().collect::<Vec<_>>().join(", ")
    ));
    true
}

fn sqldelight(path: &str, reach: BTreeSet<String>, ctx: &Ctx, out: &mut Routing) -> bool {
    let text = format!(
        "{}\n{}",
        (ctx.read)(path).unwrap_or_default(),
        (ctx.read_base)(path).unwrap_or_default()
    );
    let base = stem(name_of(path));
    let lower = format!(
        "{}{}",
        base.get(..1).unwrap_or("").to_lowercase(),
        base.get(1..).unwrap_or("")
    );
    let mut tokens: BTreeSet<String> = [format!("{base}Queries"), format!("{lower}Queries")]
        .into_iter()
        .collect();
    let cap = |t: &str| {
        format!(
            "{}{}",
            t.get(..1).unwrap_or("").to_uppercase(),
            t.get(1..).unwrap_or("")
        )
    };
    tokens.extend(SQ_TABLE_RE.captures_iter(&text).map(|c| cap(&c[1])));
    tokens.extend(SQ_LABEL_RE.captures_iter(&text).map(|c| cap(&c[1])));
    out.mentions.push(Mention {
        tokens: tokens.into_iter().collect(),
        modules: reach,
        ..Default::default()
    });
    out.reasons.push(format!(
        "{path}: SQLDelight — seeding what names its generated API"
    ));
    true
}

fn deleted_kotlin(path: &str, reach: BTreeSet<String>, ctx: &Ctx, out: &mut Routing) -> bool {
    if (ctx.read)(path).is_some() {
        return false; // present but unindexed: nothing to reason from
    }
    let Some(text) = (ctx.read_base)(path) else {
        return false;
    };
    let raw = crate::parse::parse_file(&text, false);
    let names: BTreeSet<String> = raw
        .decls
        .iter()
        .filter(|d| d.parent < 0 && !d.name.is_empty())
        .map(|d| d.name.clone())
        .collect();
    if names.is_empty() {
        out.reasons
            .push(format!("{path}: deleted, declared nothing"));
        return true;
    }
    out.mentions.push(Mention {
        tokens: names.into_iter().collect(),
        modules: reach,
        package: Some(raw.package.clone()),
        ..Default::default()
    });
    out.reasons.push(format!(
        "{path}: deleted — seeding what still writes its names"
    ));
    true
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Byte ranges of string-literal content in `text` (kt-syntax tokens); None when it won't parse.
fn string_spans(text: &str) -> Option<Vec<(usize, usize)>> {
    use kt_syntax::SyntaxKind as K;
    let file = kt_syntax::parse(text).ok()?;
    let mut out = Vec::new();
    for t in file.root().tokens() {
        let k = t.kind();
        if matches!(
            k,
            K::REGULAR_STRING_PART | K::CLOSING_QUOTE | K::OPEN_QUOTE | K::ESCAPE_SEQUENCE
        ) {
            let r = t.range();
            out.push((r.start, r.end));
        }
    }
    out.sort();
    Some(out)
}

fn sees_package(text: &str, file_pkg: &str, pkg: &str) -> bool {
    if file_pkg == pkg {
        return true;
    }
    let prefix = format!("{pkg}.");
    text.lines()
        .filter_map(|l| l.trim_start().strip_prefix("import "))
        .any(|imp| {
            let imp = imp.trim().replace('`', "");
            let star = imp.ends_with(".*");
            let fqn = imp
                .trim_end_matches(".*")
                .split(" as ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            fqn.starts_with(&prefix) || (star && fqn == pkg)
        })
}

/// path -> one-line hunks on every line that writes a mentioned token.
pub fn mention_hunks(
    mentions: &[Mention],
    files: &[(&str, Option<&str>)],
    read: &(dyn Fn(&str) -> Option<String> + Sync),
) -> BTreeMap<String, Vec<Hunk>> {
    use rayon::prelude::*;
    if mentions.is_empty() {
        return BTreeMap::new();
    }
    let found: Vec<(String, Vec<Hunk>)> = files
        .par_iter()
        .filter_map(|(path, module)| {
            let live: Vec<&Mention> = mentions
                .iter()
                .filter(|m| module.map(|id| m.modules.contains(id)).unwrap_or(false))
                .collect();
            if live.is_empty() {
                return None;
            }
            let text = read(path)?;
            if !live
                .iter()
                .any(|m| m.tokens.iter().any(|t| text.contains(t.as_str())))
            {
                return None;
            }
            let bytes = text.as_bytes();
            let (file_pkg, header) = package_line(&text);
            let starts: Vec<usize> = std::iter::once(0)
                .chain(text.match_indices('\n').map(|(i, _)| i + 1))
                .collect();
            let mut spans: Option<Option<Vec<(usize, usize)>>> = None;
            let mut lines: BTreeSet<usize> = BTreeSet::new();
            for m in &live {
                if let Some(pkg) = &m.package
                    && !sees_package(&text, &file_pkg, pkg)
                {
                    continue;
                }
                if !m.file_markers.is_empty()
                    && !m.file_markers.iter().any(|mk| text.contains(mk.as_str()))
                {
                    continue;
                }
                for tok in &m.tokens {
                    for (at, _) in text.match_indices(tok.as_str()) {
                        let end = at + tok.len();
                        if (at > 0 && is_word(bytes[at - 1]))
                            || (end < bytes.len() && is_word(bytes[end]))
                        {
                            continue;
                        }
                        if header.0 <= at && at < header.1 {
                            continue;
                        }
                        if m.string_only {
                            let s = spans.get_or_insert_with(|| string_spans(&text));
                            // Unparseable: every hit counts (sound).
                            if let Some(s) = s
                                && !s.iter().any(|&(a, b)| a <= at && end <= b)
                            {
                                continue;
                            }
                        }
                        lines.insert(starts.partition_point(|&s| s <= at));
                    }
                }
            }
            if lines.is_empty() {
                return None;
            }
            let tl: Vec<&str> = text.split('\n').collect();
            let hunks = lines
                .into_iter()
                .map(|n| Hunk {
                    start: n as u32,
                    count: 1,
                    old_start: 0,
                    removed: vec![],
                    added: vec![tl[n - 1].to_string()],
                })
                .collect();
            Some((path.to_string(), hunks))
        })
        .collect();
    found.into_iter().collect()
}

fn package_line(text: &str) -> (String, (usize, usize)) {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("package ") {
            let pkg = rest.trim().trim_end_matches(';').replace('`', "");
            return (pkg, (at, at + line.len()));
        }
        if !(t.is_empty()
            || t.starts_with("//")
            || t.starts_with("/*")
            || t.starts_with('*')
            || t.starts_with('@'))
        {
            break;
        }
        at += line.len();
    }
    (String::new(), (0, 0))
}

/// The cheapest-looking class: fewest lines, preferring one whose file names a marker.
pub fn smoke_class<'a>(
    classes: &'a BTreeSet<String>,
    class_path: &BTreeMap<&str, (&str, u32)>,
    read: &dyn Fn(&str) -> Option<String>,
    markers: &[String],
) -> Option<&'a String> {
    let mut ranked: Vec<&String> = classes.iter().collect();
    ranked.sort_by_key(|c| {
        (
            class_path.get(c.as_str()).map(|x| x.1).unwrap_or(1 << 30),
            (*c).clone(),
        )
    });
    if !markers.is_empty() {
        for c in &ranked {
            let text = class_path
                .get(c.as_str())
                .and_then(|(p, _)| read(p))
                .unwrap_or_default();
            if markers.iter().any(|m| text.contains(m.as_str())) {
                return Some(c);
            }
        }
    }
    ranked.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_names_its_tables_not_sql_words() {
        let t = migration_tables(
            "-- users only in this comment\nALTER TABLE task_links ADD COLUMN note text;\nINSERT INTO public.audit_log VALUES (1);",
        );
        assert_eq!(
            t,
            ["audit_log", "task_links"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        );
    }

    #[test]
    fn accessor_resources_drop_their_type_directory() {
        assert_eq!(
            resource_tokens("drawable/fa_circle.xml", "composeResources"),
            vec!["drawable/fa_circle.xml", "fa_circle", "fa_circle.xml"]
        );
        assert!(
            resource_tokens("accounting-export/legacy.csv", "resources")
                .contains(&"accounting-export".to_string())
        );
    }

    #[test]
    fn string_only_mentions_skip_code_identifiers() {
        let m = vec![Mention {
            tokens: vec!["index".into()],
            modules: ["m".to_string()].into_iter().collect(),
            string_only: true,
            ..Default::default()
        }];
        let read = |p: &str| -> Option<String> {
            Some(match p {
                "a/A.kt" => "package x\nval index = 1\n".to_string(),
                _ => "package x\nval f = load(\"index\")\n".to_string(),
            })
        };
        let got = mention_hunks(&m, &[("a/A.kt", Some("m")), ("a/B.kt", Some("m"))], &read);
        assert_eq!(got.keys().collect::<Vec<_>>(), vec!["a/B.kt"]);
        assert_eq!(got["a/B.kt"][0].start, 2);
    }
}

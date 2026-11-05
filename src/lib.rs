//! kt-affected: which tests can a change to a Kotlin/Gradle project affect?
//!
//! Discovers the Gradle/KMP build statically, keeps a declaration-level reference graph of every
//! `.kt` file in an on-disk cache, and turns a git diff into the Gradle test tasks and classes that
//! can observe it.
//!
//! ```no_run
//! use std::path::Path;
//!
//! let opts = kt_affected::SelectOptions::new().base("origin/main");
//! match kt_affected::select(Path::new("."), &opts) {
//!     Ok(sel) => println!("./gradlew {}", sel.to_gradle_args().join(" ")),
//!     Err(e) => eprintln!("run everything: {e}"),
//! }
//! ```
//!
//! # Memory
//!
//! Nothing stays resident between calls: each call opens the graph cache (memory-mapped), brings it
//! up to date, computes, and drops everything before it returns. Parallel work runs on rayon's
//! global thread pool unless [`SelectOptions::threads`] asks for a pool of its own for the call.
//! See `docs/library.md`.

mod cache;
mod change;
mod config;
mod conventions;
mod discover;
mod git;
mod graph;
mod koin;
mod parse;
mod route;
mod seeds;
mod select;

#[cfg(feature = "cli")]
mod bench;
#[cfg(feature = "cli")]
#[doc(hidden)]
pub mod cli;

pub use select::Group;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// What to compare and where the build's inputs live. Every field has a default; set the ones you
/// need with the builder methods.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SelectOptions {
    /// The commit the change is measured from. `None`: [`default_base`].
    pub base: Option<String>,
    /// The commit the change ends at. `None`: the work tree, uncommitted changes included.
    pub head: Option<String>,
    /// The git repository. `None`: the build root.
    pub git_dir: Option<PathBuf>,
    /// The `kt-affected.toml` to use. `None`: the build root's, else the defaults.
    pub config: Option<PathBuf>,
    /// Where the graph cache lives. `None`: `<build root>/build/kt-affected`.
    pub cache_dir: Option<PathBuf>,
    /// Use static discovery only, never a cached Gradle export.
    pub static_discovery: bool,
    /// Run the call on a thread pool of this size, dropped when it returns. `None`: rayon's
    /// global pool.
    pub threads: Option<usize>,
}

impl SelectOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }
    pub fn head(mut self, head: impl Into<String>) -> Self {
        self.head = Some(head.into());
        self
    }
    pub fn git_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.git_dir = Some(dir.into());
        self
    }
    pub fn config(mut self, file: impl Into<PathBuf>) -> Self {
        self.config = Some(file.into());
        self
    }
    pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }
    pub fn static_discovery(mut self, on: bool) -> Self {
        self.static_discovery = on;
        self
    }
    pub fn threads(mut self, n: usize) -> Self {
        self.threads = Some(n);
        self
    }
}

/// The test plan for a change: what `kt-affected select --format json` prints.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Selection {
    /// The resolved base commit.
    pub base: String,
    /// Every path the diff touches, deletions included.
    pub changed_files: Vec<String>,
    /// Why groups run whole, or why something was left out.
    pub reasons: Vec<String>,
    /// The test tasks to run, in a stable order.
    pub groups: Vec<Group>,
    /// Always `None` in a returned `Selection` (a failure is an [`Error`]); the CLI fills it in
    /// the JSON it prints when it cannot answer.
    #[serde(default)]
    pub error: Option<String>,
}

impl Selection {
    /// Arguments for `./gradlew`: each local group's task path, followed by `--tests <class>` for
    /// each class unless the whole task runs. Non-local groups are left out. What
    /// `select --format gradle` prints, joined by spaces.
    pub fn to_gradle_args(&self) -> Vec<String> {
        let mut out = Vec::new();
        for g in self.groups.iter().filter(|g| g.local) {
            out.push(if g.module == ":" {
                format!(":{}", g.task)
            } else {
                format!("{}:{}", g.module, g.task)
            });
            if !g.full {
                for c in &g.classes {
                    out.push("--tests".into());
                    out.push(c.clone());
                }
            }
        }
        out
    }

    /// One `<module>:<task>\t<class>` line per selected class, every group included. What
    /// `select --format classes` prints, joined by newlines.
    pub fn to_classes(&self) -> Vec<String> {
        self.groups
            .iter()
            .flat_map(|g| {
                g.classes
                    .iter()
                    .map(move |c| format!("{}:{}\t{c}", g.module, g.task))
            })
            .collect()
    }
}

/// One changed declaration and whether it seeds the selection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Verdict {
    /// Path of the changed file, relative to the build root.
    pub file: String,
    /// The declaration (`pkg.Class.member`), or `(whole file)`.
    pub declaration: String,
    /// Its first line (1-based; 0 for a whole file).
    pub line: u32,
    /// A functional change seeds the selection; a cosmetic one does not.
    pub functional: bool,
    pub reason: String,
}

/// One step on a test's path to a seed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Step {
    pub declaration: String,
    /// How this declaration was reached from the next step (toward the seed).
    pub how: String,
}

/// How the graph reaches one selected test class.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TestPath {
    /// Fully qualified test class.
    pub test: String,
    /// From the test back to a seed: the first step is in the test, the last is the seed.
    pub steps: Vec<Step>,
}

/// Why a selection is what it is: what `kt-affected explain` prints, as data.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Explanation {
    pub selection: Selection,
    /// Per changed declaration: functional or cosmetic, and why.
    pub verdicts: Vec<Verdict>,
    /// Per test class the graph reached. Classes selected by a path, scanner or smoke rule have
    /// none.
    pub paths: Vec<TestPath>,
}

impl Explanation {
    /// The `explain` text.
    pub fn render(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Explanation {
    fn fmt(&self, o: &mut fmt::Formatter<'_>) -> fmt::Result {
        let plan = &self.selection;
        writeln!(o, "changed declarations:")?;
        if self.verdicts.is_empty() {
            writeln!(
                o,
                "  none: every Kotlin hunk touches only comments, KDoc, blank lines or imports"
            )?;
        }
        for v in &self.verdicts {
            let verdict = if v.functional {
                "functional"
            } else {
                "cosmetic  "
            };
            writeln!(
                o,
                "  {verdict} {} ({}:{}): {}",
                v.declaration, v.file, v.line, v.reason
            )?;
        }
        if !plan.reasons.is_empty() {
            writeln!(o, "rules:")?;
            for r in &plan.reasons {
                writeln!(o, "  {r}")?;
            }
        }
        let paths: std::collections::BTreeMap<&str, &[Step]> = self
            .paths
            .iter()
            .map(|p| (p.test.as_str(), p.steps.as_slice()))
            .collect();
        let n: usize = plan.groups.iter().map(|g| g.classes.len()).sum();
        writeln!(
            o,
            "selected tests ({n} classes in {} groups):",
            plan.groups.len()
        )?;
        for g in &plan.groups {
            writeln!(
                o,
                "  {}:{}{}",
                g.module,
                g.task,
                if g.full { " (whole task)" } else { "" }
            )?;
            if g.full {
                continue;
            }
            for c in &g.classes {
                match paths.get(c.as_str()) {
                    Some(steps) => {
                        let Some(seed) = steps.last() else { continue };
                        write!(o, "    {c}\n      {}", seed.declaration)?;
                        // Each step records how it was reached from the step before it (toward the seed).
                        for k in (0..steps.len() - 1).rev() {
                            write!(
                                o,
                                "\n        -[{}]-> {}",
                                steps[k].how, steps[k].declaration
                            )?;
                        }
                        writeln!(o)?;
                    }
                    None => writeln!(
                        o,
                        "    {c}  (selected by a path, scanner or smoke rule, not the graph)"
                    )?,
                }
            }
        }
        Ok(())
    }
}

/// Why no selection could be made. Never answer "no tests" on an error: run everything.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// No `settings.gradle(.kts)` at or above the given directory.
    NoBuildRoot(PathBuf),
    /// A git command failed, or no base was given and none could be resolved.
    Git(String),
    /// `kt-affected.toml` could not be read or parsed.
    Config(String),
    /// The graph cache could not be built, updated or opened.
    Graph(String),
    /// The selector cannot tell which tests the change reaches.
    CouldNotAnswer(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoBuildRoot(dir) => write!(
                f,
                "kt-affected: no settings.gradle(.kts) at or above {}",
                dir.display()
            ),
            Error::Git(m) | Error::Config(m) | Error::Graph(m) | Error::CouldNotAnswer(m) => {
                f.write_str(m)
            }
        }
    }
}

impl std::error::Error for Error {}

/// The base `select` uses when [`SelectOptions::base`] is `None`: the merge-base of `head` (HEAD
/// when `None`) with the remote's default branch, `origin/HEAD`, else `origin/main`, else
/// `origin/master`.
pub fn default_base(git_dir: &Path, head: Option<&str>) -> Result<String, Error> {
    git::Repo::default_upstream(git_dir)
        .and_then(|up| git::Repo::merge_base(git_dir, &up, head))
        .map_err(Error::Git)
}

/// The test plan for the change from `opts.base` to `opts.head` in the Gradle build at or above
/// `root`. Updates the graph cache as it goes.
pub fn select(root: &Path, opts: &SelectOptions) -> Result<Selection, Error> {
    run(root, opts, false).map(|p| selection(p).0)
}

/// [`select`], with why each changed declaration counts and how each selected test is reached.
pub fn explain(root: &Path, opts: &SelectOptions) -> Result<Explanation, Error> {
    run(root, opts, true).map(|p| {
        let (selection, e) = selection(p);
        let e = e.unwrap_or_default();
        Explanation {
            selection,
            verdicts: e
                .verdicts
                .into_iter()
                .map(|(file, declaration, line, functional, reason)| Verdict {
                    file,
                    declaration,
                    line,
                    functional,
                    reason,
                })
                .collect(),
            paths: e
                .paths
                .into_iter()
                .map(|(test, steps)| TestPath {
                    test,
                    steps: steps
                        .into_iter()
                        .map(|(declaration, how)| Step { declaration, how })
                        .collect(),
                })
                .collect(),
        }
    })
}

fn selection(p: select::Plan) -> (Selection, Option<select::Explain>) {
    (
        Selection {
            base: p.base,
            changed_files: p.changed_files,
            reasons: p.reasons,
            groups: p.groups,
            error: None,
        },
        p.explain,
    )
}

fn run(root: &Path, opts: &SelectOptions, explain: bool) -> Result<select::Plan, Error> {
    match opts.threads {
        Some(n) => match rayon::ThreadPoolBuilder::new().num_threads(n).build() {
            Ok(pool) => pool.install(|| plan(root, opts, explain)),
            Err(e) => Err(Error::Graph(format!("thread pool: {e}"))),
        },
        None => plan(root, opts, explain),
    }
}

fn plan(root: &Path, opts: &SelectOptions, explain: bool) -> Result<select::Plan, Error> {
    let root = find_build_root(root).ok_or_else(|| Error::NoBuildRoot(root.to_path_buf()))?;
    let cfg = match &opts.config {
        Some(f) => config::Config::load_file(f),
        None => config::Config::load(&root),
    }
    .map_err(Error::Config)?;
    let git_dir = opts.git_dir.clone().unwrap_or_else(|| root.clone());
    let base = match &opts.base {
        Some(b) => b.clone(),
        None => default_base(&git_dir, opts.head.as_deref())?,
    };
    let repo =
        git::Repo::new(git_dir, root.clone(), &base, opts.head.clone()).map_err(Error::Git)?;
    let dir = opts
        .cache_dir
        .clone()
        .unwrap_or_else(|| root.join("build/kt-affected"));
    let build = load_build(&root, &dir, opts.static_discovery);
    let (_, packed) =
        cache::update_open(&root, &build, &dir, false).map_err(|e| Error::Graph(e.to_string()))?;
    let packed = packed.ok_or_else(|| Error::Graph("no graph".into()))?;
    select::select(&select::Inputs {
        repo: &repo,
        build: &build,
        packed: &packed,
        cfg: &cfg,
        explain,
    })
    .map_err(Error::CouldNotAnswer)
}

/// The nearest directory at or above `start` holding a settings file, as Gradle resolves its build.
pub(crate) fn find_build_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("settings.gradle.kts").is_file() || d.join("settings.gradle").is_file())
        .map(Path::to_path_buf)
}

/// The cached discovery when its inputs are unchanged, else a fresh one (static, refined by a
/// current Gradle export unless `static_only`).
pub(crate) fn load_build(root: &Path, dir: &Path, static_only: bool) -> discover::Build {
    if let Some(p) = cache::Packed::open(&dir.join("graph.bin"))
        && let Ok(b) = serde_json::from_slice::<discover::Build>(p.discovery())
        && cache::discovery_stamp(root, &b.inputs) == p.meta(1)
    {
        return b;
    }
    if static_only {
        discover::discover(root)
    } else {
        discover::discover_best(root)
    }
}

#[cfg(test)]
mod root_tests {
    use super::find_build_root;

    #[test]
    fn a_subdirectory_resolves_to_the_build_that_contains_it() {
        let tmp = std::env::temp_dir().join(format!("kt-affected-root-{}", std::process::id()));
        let nested = tmp.join("tools/kt-affected/src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(tmp.join("settings.gradle.kts"), "include(\":a\")\n").unwrap();
        assert_eq!(Some(tmp.clone()), find_build_root(&nested));
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn no_settings_file_anywhere_up_the_tree_is_no_build() {
        assert_eq!(None, find_build_root(std::path::Path::new("/")));
    }
}

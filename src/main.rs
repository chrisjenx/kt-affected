//! kt-affected: which tests can a change to a Kotlin/Gradle project affect?
//!
//! Discovers the Gradle/KMP build, builds a declaration-level reference graph with kt-syntax,
//! keeps it up to date incrementally, and (in progress) selects the test classes a diff reaches.

mod bench;
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

use std::path::{Path, PathBuf};

fn usage() -> ! {
    eprintln!(
        "kt-affected {}\nusage: kt-affected <command> [--root DIR] [--cache-dir DIR]\n\
         commands:\n  \
           discover [--gradle-export | --export-dir DIR | --reference-json FILE] [--json [--static]]\n  \
           graph [--cold] [--json] [--static-discovery]   build or update the graph\n  \
           verify                           incremental graph == cold rebuild?\n  \
           preview --changed FILE           test classes reachable from the listed files\n  \
           init-script                      print the Gradle export init script\n  \
           dump-edges --out FILE            canonical edge dump, for diffing two graphs\n  \
           select --base REF [--head REF] [--git-dir DIR] [--config FILE] [--format json|gradle|classes]\n  \
                                            the test plan for base..head (or base..work tree)\n  \
           explain --base REF [same options as select]\n  \
                                            per changed declaration: functional or cosmetic and why;\n  \
                                            per selected test: the path from a seed, edge by edge\n\
         --base defaults to the merge-base with origin/HEAD (else origin/main, else origin/master).\n\
         --version prints the version.",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2)
}

/// The nearest directory at or above `start` holding a settings file, as Gradle resolves its build.
fn find_build_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("settings.gradle.kts").is_file() || d.join("settings.gradle").is_file())
        .map(Path::to_path_buf)
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

struct Args(Vec<String>);

impl Args {
    fn get(&self, flag: &str) -> Option<String> {
        self.0
            .iter()
            .position(|a| a == flag)
            .and_then(|i| self.0.get(i + 1).cloned())
    }
    fn has(&self, flag: &str) -> bool {
        self.0.iter().any(|a| a == flag)
    }
}

/// The cached discovery when its inputs are unchanged, else a fresh one (static, refined by a
/// current Gradle export unless `static_only`).
fn load_build(root: &Path, dir: &Path, static_only: bool) -> discover::Build {
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

#[derive(serde::Deserialize)]
struct RefProject {
    path: String,
    dir: String,
    deps: Vec<(String, String)>,
}

#[derive(serde::Deserialize)]
struct RefBuild {
    projects: Vec<RefProject>,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = argv.first().cloned() else {
        usage()
    };
    match cmd.as_str() {
        "--version" | "-V" | "version" => {
            println!("kt-affected {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        "--help" | "-h" | "help" => usage(),
        _ => {}
    }
    let args = Args(argv);
    let root = match args.get("--root") {
        Some(r) => {
            let r = PathBuf::from(r);
            r.canonicalize().unwrap_or(r)
        }
        None => {
            let cwd = std::env::current_dir().expect("current directory");
            match find_build_root(&cwd) {
                Some(r) => r,
                None if cmd == "init-script" => cwd,
                None => {
                    eprintln!(
                        "kt-affected: no settings.gradle(.kts) at or above {}; run inside a Gradle build or pass --root",
                        cwd.display()
                    );
                    std::process::exit(2);
                }
            }
        }
    };
    let dir = args
        .get("--cache-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build/kt-affected"));
    match cmd.as_str() {
        "init-script" => print!("{}", discover::INIT_SCRIPT),
        "discover" => {
            let t = std::time::Instant::now();
            let stat = discover::discover(&root);
            let ms = t.elapsed().as_millis();
            let other = if args.has("--gradle-export") {
                Some(discover::gradle_export(&root).unwrap_or_else(|e| panic!("{e}")))
            } else if let Some(d) = args.get("--export-dir") {
                Some(discover::read_export(&root, Path::new(&d)).unwrap_or_else(|e| panic!("{e}")))
            } else if let Some(f) = args.get("--reference-json") {
                let r: RefBuild = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
                Some(discover::Build {
                    root_name: String::new(),
                    projects: r
                        .projects
                        .into_iter()
                        .map(|p| discover::Project {
                            path: p.path,
                            dir: p.dir,
                            build_file: None,
                            deps: p.deps.into_iter().collect(),
                            source_sets: Default::default(),
                            targets: Default::default(),
                            test_tasks: Default::default(),
                            implicit: false,
                            dep_confidence: Default::default(),
                            test_filters: Default::default(),
                        })
                        .collect(),
                    included_builds: vec![],
                    inputs: vec![],
                    warnings: vec![],
                })
            } else {
                None
            };
            let edges: usize = stat.projects.iter().map(|p| p.deps.len()).sum();
            eprintln!(
                "static discovery: {} projects ({} implicit), {} dep declarations, {} included builds, {} warnings, {ms} ms",
                stat.projects.len(),
                stat.projects.iter().filter(|p| p.implicit).count(),
                edges,
                stat.included_builds.len(),
                stat.warnings.len()
            );
            if let Some(o) = other {
                let c = discover::compare(
                    &stat,
                    &o,
                    args.has("--gradle-export") || args.has("--export-dir"),
                );
                println!("{}", serde_json::to_string_pretty(&c).unwrap());
            } else if args.has("--json") {
                let best = if args.has("--static") {
                    stat
                } else {
                    discover::discover_best(&root)
                };
                println!("{}", serde_json::to_string_pretty(&best).unwrap());
            } else {
                for w in &stat.warnings {
                    eprintln!("warning: {w}");
                }
            }
        }
        "graph" => {
            let build = load_build(&root, &dir, args.has("--static-discovery"));
            let r = cache::update(&root, &build, &dir, args.has("--cold"))
                .unwrap_or_else(|e| panic!("{e}"));
            eprintln!(
                "{}: files={} nodes={} edges={} reparsed={} relinked={} walk={}ms parse={}ms link={}ms write={}ms total={}ms",
                r.mode,
                r.files,
                r.nodes,
                r.edges,
                r.reparsed,
                r.relinked,
                r.walk_ms,
                r.parse_ms,
                r.link_ms,
                r.write_ms,
                r.total_ms
            );
            if args.has("--json") {
                println!("{}", serde_json::to_string(&r).unwrap());
            }
        }
        "verify" => {
            let build = load_build(&root, &dir, args.has("--static-discovery"));
            cache::update(&root, &build, &dir, false).unwrap_or_else(|e| panic!("{e}"));
            let (same, only_inc, only_cold) =
                bench::verify(&root, &build, &dir, &dir.join("verify"))
                    .unwrap_or_else(|e| panic!("{e}"));
            println!(
                "{{\"same\":{same},\"only_incremental\":{only_inc},\"only_cold\":{only_cold}}}"
            );
            if only_inc + only_cold > 0 {
                std::process::exit(1);
            }
        }
        "preview" => {
            let build = load_build(&root, &dir, args.has("--static-discovery"));
            cache::update(&root, &build, &dir, false).unwrap_or_else(|e| panic!("{e}"));
            let p = cache::Packed::open(&dir.join("graph.bin")).expect("graph");
            let changed: Vec<String> =
                std::fs::read_to_string(args.get("--changed").unwrap_or_else(|| usage()))
                    .unwrap()
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
            for vd in [true, false] {
                let (seeds, roots) = bench::preview(&p, &changed, vd);
                println!("virtual_dispatch={vd} seeds={seeds} test_classes={roots}");
            }
        }
        "dump-edges" => {
            let build = load_build(&root, &dir, args.has("--static-discovery"));
            cache::update(&root, &build, &dir, false).unwrap_or_else(|e| panic!("{e}"));
            let p = cache::Packed::open(&dir.join("graph.bin")).expect("graph");
            let out = args.get("--out").unwrap_or_else(|| usage());
            let lines = cache::dump_edges(&p);
            std::fs::write(&out, lines.join("\n") + "\n").unwrap();
            eprintln!("rust: {} nodes, {} lines", p.n_nodes(), lines.len());
        }
        "select" | "explain" => {
            let loaded = match args.get("--config") {
                Some(f) => config::Config::load_file(Path::new(&f)),
                None => config::Config::load(&root),
            };
            let cfg = loaded.unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(3)
            });
            let git_dir = args
                .get("--git-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.clone());
            let head = args.get("--head");
            let base = match args.get("--base") {
                Some(b) => b,
                None => git::Repo::default_upstream(&git_dir)
                    .and_then(|up| git::Repo::merge_base(&git_dir, &up, head.as_deref()))
                    .unwrap_or_else(|e| {
                        eprintln!("{e}");
                        std::process::exit(3)
                    }),
            };
            let fail = |e: String| -> ! {
                println!(
                    "{}",
                    serde_json::json!({"base": base, "changedFiles": [], "reasons": [], "groups": [], "error": e})
                );
                std::process::exit(3)
            };
            let repo =
                git::Repo::new(git_dir, root.clone(), &base, head).unwrap_or_else(|e| fail(e));
            let build = load_build(&root, &dir, args.has("--static-discovery"));
            cache::update(&root, &build, &dir, false).unwrap_or_else(|e| fail(e.to_string()));
            let p = cache::Packed::open(&dir.join("graph.bin"))
                .unwrap_or_else(|| fail("no graph".into()));
            let t = std::time::Instant::now();
            let explain = cmd == "explain";
            let plan = select::select(&select::Inputs {
                repo: &repo,
                build: &build,
                packed: &p,
                cfg: &cfg,
                explain,
            })
            .unwrap_or_else(|e| fail(e));
            eprintln!(
                "select: {} groups, {} classes, {} ms",
                plan.groups.len(),
                plan.groups.iter().map(|g| g.classes.len()).sum::<usize>(),
                t.elapsed().as_millis()
            );
            if explain {
                print!("{}", select::render_explain(&plan));
            } else {
                println!(
                    "{}",
                    select::render(
                        &plan,
                        &args.get("--format").unwrap_or_else(|| "json".into())
                    )
                );
            }
        }
        _ => usage(),
    }
}

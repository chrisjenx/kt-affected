//! The `kt-affected` command line. Not part of the library's API: `src/main.rs` calls `main`.

use crate::{bench, cache, discover};
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

/// The `kt-affected` command line.
pub fn main() {
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
            match crate::find_build_root(&cwd) {
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
            let build = crate::load_build(&root, &dir, args.has("--static-discovery"));
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
            let build = crate::load_build(&root, &dir, args.has("--static-discovery"));
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
            let build = crate::load_build(&root, &dir, args.has("--static-discovery"));
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
            let build = crate::load_build(&root, &dir, args.has("--static-discovery"));
            cache::update(&root, &build, &dir, false).unwrap_or_else(|e| panic!("{e}"));
            let p = cache::Packed::open(&dir.join("graph.bin")).expect("graph");
            let out = args.get("--out").unwrap_or_else(|| usage());
            let lines = cache::dump_edges(&p);
            std::fs::write(&out, lines.join("\n") + "\n").unwrap();
            eprintln!("rust: {} nodes, {} lines", p.n_nodes(), lines.len());
        }
        "select" | "explain" => {
            let git_dir = args
                .get("--git-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.clone());
            let head = args.get("--head");
            // Resolved here, not by the library, so a failure after it can still name the base.
            let base = args
                .get("--base")
                .or_else(|| crate::default_base(&git_dir, head.as_deref()).ok());
            let mut opts = crate::SelectOptions::new()
                .git_dir(git_dir)
                .cache_dir(dir)
                .static_discovery(args.has("--static-discovery"));
            opts.base = base.clone();
            opts.head = head;
            opts.config = args.get("--config").map(PathBuf::from);
            let fail = |e: crate::Error| -> ! {
                match (&e, &base) {
                    // A config error, or no base to compare with: the message alone, as ever.
                    (crate::Error::Config(_), _) | (_, None) => eprintln!("{e}"),
                    (_, Some(base)) => println!(
                        "{}",
                        serde_json::json!({"base": base, "changedFiles": [], "reasons": [], "groups": [], "error": e.to_string()})
                    ),
                }
                std::process::exit(3)
            };
            let t = std::time::Instant::now();
            let report = |groups: &[crate::Group]| {
                eprintln!(
                    "select: {} groups, {} classes, {} ms",
                    groups.len(),
                    groups.iter().map(|g| g.classes.len()).sum::<usize>(),
                    t.elapsed().as_millis()
                )
            };
            if cmd == "explain" {
                let e = crate::explain(&root, &opts).unwrap_or_else(|e| fail(e));
                report(&e.selection.groups);
                print!("{e}");
            } else {
                let sel = crate::select(&root, &opts).unwrap_or_else(|e| fail(e));
                report(&sel.groups);
                let out = match args.get("--format").as_deref() {
                    Some("classes") => sel.to_classes().join("\n"),
                    Some("gradle") => sel.to_gradle_args().join(" "),
                    _ => serde_json::to_string_pretty(&sel).unwrap(),
                };
                println!("{out}");
            }
        }
        _ => usage(),
    }
}

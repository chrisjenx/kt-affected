//! Several processes and threads updating one cache dir at once, after each of a run of edits: every
//! caller gets the plan a fresh cold cache gives, and the shared graph still equals a cold build.
//! `KT_AFFECTED_RACE_ROUNDS` sets the number of edit rounds (default 3) for a longer local soak.

use kt_affected::SelectOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const FILES: usize = 300;
const CALLERS: usize = 6;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

fn class(i: usize, bump: usize) -> String {
    let dep = if i == 0 {
        "0".to_string()
    } else {
        format!("C{}().f()", i / 2)
    };
    format!("package p\n\nclass C{i} {{\n    fun f(): Int = {dep} + {bump}\n}}\n")
}

/// A binary tree of classes, each calling its parent, with a test on every fourth.
fn fixture(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kt-affected-race-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    write(&root, "settings.gradle.kts", "include(\":m\")\n");
    write(&root, ".gitignore", "build/\n");
    write(&root, "m/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
    for i in 0..FILES {
        write(&root, &format!("m/src/main/kotlin/p/C{i}.kt"), &class(i, 1));
        if i % 4 == 0 {
            write(
                &root,
                &format!("m/src/test/kotlin/p/T{i}.kt"),
                &format!(
                    "package p\n\nimport kotlin.test.Test\n\nclass T{i} {{\n    @Test\n    fun t() {{\n        C{i}().f()\n    }}\n}}\n"
                ),
            );
        }
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    root
}

/// Round `r`'s edit: a body change (the delta path), and on odd rounds a new file (a full relink,
/// which reads every carried record back from `parses.bin`).
fn edit(root: &Path, r: usize) {
    let i = (r * 37 + 5) % FILES;
    write(
        root,
        &format!("m/src/main/kotlin/p/C{i}.kt"),
        &class(i, r + 100),
    );
    if r % 2 == 1 {
        let parent = (r * 53) % FILES;
        write(
            root,
            &format!("m/src/main/kotlin/p/N{r}.kt"),
            &format!("package p\n\nclass N{r} {{\n    fun g(): Int = C{parent}().f()\n}}\n"),
        );
    }
}

fn select_cmd(root: &Path, cache: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_kt-affected"));
    c.args(["select", "--base", "HEAD", "--format", "classes", "--root"])
        .arg(root)
        .arg("--cache-dir")
        .arg(cache);
    c
}

fn plan(out: std::process::Output) -> String {
    assert!(
        out.status.success(),
        "select failed ({}): {}{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn concurrent_selects_on_one_cache_agree_with_a_cold_cache() {
    let rounds: usize = std::env::var("KT_AFFECTED_RACE_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let root = fixture("select");
    let shared = root.join("build/shared");
    for r in 0..rounds {
        // Round 0 races cold builds; later rounds race updates after an edit.
        if r > 0 {
            edit(&root, r);
        }
        let reference = plan(
            select_cmd(&root, &root.join(format!("build/cold-{r}")))
                .output()
                .unwrap(),
        );
        assert!(
            !reference.is_empty(),
            "round {r}: the fixture selects nothing"
        );

        let procs: Vec<_> = (0..CALLERS)
            .map(|_| {
                select_cmd(&root, &shared)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let threads: Vec<_> = (0..CALLERS / 2)
            .map(|_| {
                let (root, shared) = (root.clone(), shared.clone());
                std::thread::spawn(move || {
                    let opts = SelectOptions::new().base("HEAD").cache_dir(&shared);
                    kt_affected::select(&root, &opts).map(|s| s.to_classes().join("\n") + "\n")
                })
            })
            .collect();
        for p in procs {
            assert_eq!(
                plan(p.wait_with_output().unwrap()),
                reference,
                "round {r}: a process's plan differs from a cold cache's"
            );
        }
        for t in threads {
            let got = t
                .join()
                .unwrap()
                .unwrap_or_else(|e| panic!("round {r}: {e}"));
            assert_eq!(got, reference, "round {r}: a thread's plan differs");
        }
        let stray: Vec<_> = std::fs::read_dir(&shared)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "round {r}: temp files left: {stray:?}");
    }

    let out = Command::new(env!("CARGO_BIN_EXE_kt-affected"))
        .args(["verify", "--root"])
        .arg(&root)
        .arg("--cache-dir")
        .arg(&shared)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "verify: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}

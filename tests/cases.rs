//! The edge-case corpus: every directory under `tests/cases/` is one selection scenario.
//!
//! `base/` is committed, `head/` is laid over it (a file holding exactly `<deleted>` deletes the
//! path) and committed, and the real binary selects `HEAD~1..HEAD`. `expect.toml` says what must
//! and must never be selected. A config lives outside the repo, since one inside is a selector
//! change. `KT_CASES=name,name` runs only those cases; `KT_CASES_SHOW=1` prints every plan.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    /// One line: the behaviour this case pins.
    why: String,
    /// `pkg.Class`, or `pkg.Class@:module:task` to pin the task.
    #[serde(default)]
    must: Vec<String>,
    #[serde(default)]
    never: Vec<String>,
    /// `:module:task` groups that must run whole.
    #[serde(default)]
    full_tasks: Vec<String>,
    /// `:module:task` groups that must run some class (a smoke class, say).
    #[serde(default)]
    tasks: Vec<String>,
    /// `:module:task` groups that must not appear at all.
    #[serde(default)]
    never_tasks: Vec<String>,
    /// Nothing at all is selected.
    #[serde(default)]
    none: bool,
    /// Each must be a substring of some reason.
    #[serde(default)]
    reasons: Vec<String>,
    /// `kt-affected.toml` text, written outside the fixture.
    config: Option<String>,
    /// The tool does not handle this yet: a failure is reported, not fatal; a pass is stale.
    known_gap: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    reasons: Vec<String>,
    groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
    module: String,
    task: String,
    classes: Vec<String>,
    full: bool,
}

impl Group {
    fn key(&self) -> String {
        format!("{}:{}", self.module, self.task)
    }
}

const DELETED: &str = "<deleted>";

fn copy_tree(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            std::fs::create_dir_all(to).unwrap();
            let text = std::fs::read(e.path()).unwrap();
            if text.trim_ascii() == DELETED.as_bytes() {
                let _ = std::fs::remove_file(&dst);
            } else {
                std::fs::write(&dst, text).unwrap();
            }
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .args([
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "-c",
            "core.autocrlf=false",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// `pkg.Class` or `pkg.Class@:module:task`.
fn split_pin(s: &str) -> (&str, Option<&str>) {
    match s.split_once('@') {
        Some((c, t)) => (c, Some(t)),
        None => (s, None),
    }
}

fn selected(plan: &Plan, class: &str, task: Option<&str>) -> bool {
    plan.groups
        .iter()
        .any(|g| task.map(|t| g.key() == t).unwrap_or(true) && g.classes.iter().any(|c| c == class))
}

fn check(e: &Expect, plan: &Plan) -> Vec<String> {
    let mut wrong = Vec::new();
    for m in &e.must {
        let (c, t) = split_pin(m);
        if !selected(plan, c, t) {
            wrong.push(format!("missing {m}"));
        }
    }
    for n in &e.never {
        let (c, t) = split_pin(n);
        if selected(plan, c, t) {
            wrong.push(format!("wrongly selected {n}"));
        }
    }
    for t in &e.full_tasks {
        if !plan.groups.iter().any(|g| g.key() == *t && g.full) {
            wrong.push(format!("{t} not run whole"));
        }
    }
    for t in &e.tasks {
        if !plan
            .groups
            .iter()
            .any(|g| g.key() == *t && !g.classes.is_empty())
        {
            wrong.push(format!("{t} runs nothing"));
        }
    }
    for t in &e.never_tasks {
        if plan.groups.iter().any(|g| g.key() == *t) {
            wrong.push(format!("{t} wrongly scheduled"));
        }
    }
    if e.none && !plan.groups.is_empty() {
        wrong.push("expected nothing selected".into());
    }
    for r in &e.reasons {
        if !plan.reasons.iter().any(|x| x.contains(r.as_str())) {
            wrong.push(format!("no reason containing {r:?}"));
        }
    }
    wrong
}

fn describe(plan: &Plan) -> String {
    let mut o = String::from("    selected:\n");
    for g in &plan.groups {
        o.push_str(&format!(
            "      {}{}: {}\n",
            g.key(),
            if g.full { " (full)" } else { "" },
            g.classes.join(", ")
        ));
    }
    o.push_str("    reasons:\n");
    for r in &plan.reasons {
        o.push_str(&format!("      {r}\n"));
    }
    o
}

/// The case's failures (empty: it passed).
fn run_case(dir: &Path, scratch: &Path) -> Result<Vec<String>, String> {
    let e: Expect = toml::from_str(
        &std::fs::read_to_string(dir.join("expect.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("expect.toml: {e}"))?;
    if e.why.trim().is_empty() {
        return Err("expect.toml: empty `why`".into());
    }
    let root = scratch.join("repo");
    let _ = std::fs::remove_dir_all(scratch);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    copy_tree(&dir.join("base"), &root);
    std::fs::write(root.join(".gitignore"), "build/\n").map_err(|e| e.to_string())?;
    git(&root, &["init", "-q"])?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-qm", "base"])?;
    if !dir.join("head").is_dir() {
        return Err("no head/".into());
    }
    copy_tree(&dir.join("head"), &root);
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-qm", "head", "--allow-empty"])?;
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kt-affected"));
    cmd.args([
        "select", "--base", "HEAD~1", "--head", "HEAD", "--format", "json",
    ])
    .arg("--root")
    .arg(&root)
    .arg("--cache-dir")
    .arg(scratch.join("cache"));
    if let Some(c) = &e.config {
        let f = scratch.join("kt-affected.toml");
        std::fs::write(&f, c).map_err(|e| e.to_string())?;
        cmd.arg("--config").arg(f);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        return Err(format!(
            "exit {:?}: {stdout}{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let plan: Plan = serde_json::from_str(&stdout).map_err(|e| format!("{e}: {stdout}"))?;
    let mut wrong = check(&e, &plan);
    if std::env::var_os("KT_CASES_SHOW").is_some() {
        eprintln!("{}:\n{}", dir.display(), describe(&plan));
    }
    if !wrong.is_empty() {
        wrong.push(describe(&plan));
    }
    let _ = std::fs::remove_dir_all(scratch); // best effort: Windows may still hold a handle
    Ok(match (e.known_gap, wrong.is_empty()) {
        (Some(gap), true) => vec![format!(
            "passes, but is marked known_gap ({gap}): remove the mark"
        )],
        (Some(gap), false) => {
            eprintln!(
                "known gap {}: {gap}\n  {}",
                dir.file_name().unwrap().to_string_lossy(),
                wrong.join("\n  ")
            );
            vec![]
        }
        (None, _) => wrong,
    })
}

#[test]
fn every_case_selects_what_it_expects() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let only: Option<BTreeSet<String>> = std::env::var("KT_CASES")
        .ok()
        .map(|s| s.split(',').map(str::to_string).collect());
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&cases_dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            only.as_ref()
                .map(|o| o.contains(p.file_name().unwrap().to_str().unwrap()))
                .unwrap_or(true)
        })
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "no cases under {}", cases_dir.display());
    let scratch = std::env::temp_dir().join(format!("kt-affected-cases-{}", std::process::id()));
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(String, Result<Vec<String>, String>)>> = Default::default();
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(dir) = cases.get(i) else { break };
                    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
                    let r = run_case(dir, &scratch.join(&name));
                    results.lock().unwrap().push((name, r));
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&scratch); // best effort: Windows may still hold a handle
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let failed: Vec<String> = results
        .iter()
        .filter_map(|(name, r)| match r {
            Ok(w) if w.is_empty() => None,
            Ok(w) => Some(format!("{name}:\n  {}", w.join("\n  "))),
            Err(e) => Some(format!("{name}: {e}")),
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} of {} cases failed:\n{}",
        failed.len(),
        results.len(),
        failed.join("\n")
    );
}

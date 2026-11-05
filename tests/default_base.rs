//! With no --base, select compares with the remote's default branch, whichever name it has.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn repo(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "kt-affected-default-base-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("m/src/main/kotlin")).unwrap();
    std::fs::write(root.join("settings.gradle.kts"), "include(\":m\")\n").unwrap();
    std::fs::write(
        root.join("m/build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    std::fs::write(root.join("m/src/main/kotlin/A.kt"), "class A\n").unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    root
}

fn select_base(root: &Path) -> Result<String, String> {
    let out = Command::new(env!("CARGO_BIN_EXE_kt-affected"))
        .args(["select", "--root"])
        .arg(root)
        .output()
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| String::from_utf8_lossy(&out.stderr).to_string())?;
    if out.status.success() {
        Ok(plan["base"].as_str().unwrap().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

#[test]
fn the_remote_default_branch_is_the_base_whatever_it_is_called() {
    for (tag, branch, head_ref) in [
        ("head", "trunk", true),
        ("main", "main", false),
        ("master", "master", false),
    ] {
        let root = repo(tag);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(
            &root,
            &[
                "update-ref",
                &format!("refs/remotes/origin/{branch}"),
                &base,
            ],
        );
        if head_ref {
            git(
                &root,
                &[
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    &format!("refs/remotes/origin/{branch}"),
                ],
            );
        }
        std::fs::write(
            root.join("m/src/main/kotlin/A.kt"),
            "class A { val x = 1 }\n",
        )
        .unwrap();
        git(&root, &["commit", "-qam", "change"]);
        assert_eq!(Ok(base), select_base(&root), "origin/{branch}");
        let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    }
}

#[test]
fn no_remote_branch_to_compare_with_is_an_error_not_an_empty_plan() {
    let root = repo("none");
    let err = select_base(&root).unwrap_err();
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(err.contains("no --base given"), "{err}");
}

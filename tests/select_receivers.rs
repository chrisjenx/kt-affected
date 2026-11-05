//! End-to-end: the real binary against a throwaway git repo, so the whole path (discovery,
//! graph, seeds, gate, routing) answers "which tests can see this member edit?".

use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "package p

interface RepoApi {
    fun foo(): Int
    fun bar(): Int
}

class RepoImpl : RepoApi {
    override fun foo(): Int = 1
    override fun bar(): Int = 2
}
";

fn test_class(name: &str, members: &str, body: &str) -> String {
    format!(
        "package p\n\nimport kotlin.test.BeforeTest\nimport kotlin.test.Test\n\nclass {name} {{\n{members}\n\n    @Test\n    fun t() {{\n        {body}\n    }}\n}}\n"
    )
}

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

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!("kt-affected-receivers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    write(&root, "settings.gradle.kts", "include(\":m\")\n");
    write(&root, "m/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
    write(&root, "m/src/main/kotlin/p/Repo.kt", REPO);
    write(
        &root,
        "m/src/main/kotlin/p/Graph.kt",
        "package p\n\nobject Graph {\n    val repo: RepoApi = RepoImpl()\n}\n",
    );
    let t = "m/src/test/kotlin/p";
    let tests = [
        (
            "PropertyHolderTest",
            "    private val r: RepoApi = RepoImpl()",
            "r.foo()",
        ),
        (
            "LateinitTest",
            "    private lateinit var r: RepoApi\n\n    @BeforeTest\n    fun setUp() {\n        r = RepoImpl()\n    }",
            "r.foo()",
        ),
        (
            "GraphAccessorTest",
            "    private val r: RepoApi = Graph.repo",
            "r.foo()",
        ),
        (
            "FakeOnlyTest",
            "    private val r: RepoApi = object : RepoApi {\n        override fun foo() = 9\n        override fun bar() = 9\n    }",
            "r.foo()",
        ),
        ("BarOnlyTest", "", "RepoImpl().bar()"),
    ];
    for (name, members, body) in tests {
        write(
            &root,
            &format!("{t}/{name}.kt"),
            &test_class(name, members, body),
        );
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    root
}

#[test]
fn an_impl_member_edit_reaches_every_test_that_can_dispatch_to_it_and_no_other() {
    let root = fixture();
    write(
        &root,
        "m/src/main/kotlin/p/Repo.kt",
        &REPO.replace(
            "override fun foo(): Int = 1",
            "override fun foo(): Int = 41",
        ),
    );
    let out = Command::new(env!("CARGO_BIN_EXE_kt-affected"))
        .args(["select", "--base", "HEAD", "--format", "classes", "--root"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let picked: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| l.split('\t').nth(1).map(str::to_string))
        .collect();
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    // Held in a property, assigned in setUp, or handed out by a graph: each calls RepoImpl.foo.
    for must in [
        "p.PropertyHolderTest",
        "p.LateinitTest",
        "p.GraphAccessorTest",
    ] {
        assert!(
            picked.iter().any(|c| c == must),
            "{must} missing from {picked:?}"
        );
    }
    // A fake never runs RepoImpl's code, and bar() did not change.
    for never in ["p.FakeOnlyTest", "p.BarOnlyTest"] {
        assert!(
            !picked.iter().any(|c| c == never),
            "{never} wrongly in {picked:?}"
        );
    }
}

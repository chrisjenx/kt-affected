//! End-to-end: tests that reach a service only through a test graph's accessor, with the graph's
//! type inferred at the call site, so the linker cannot resolve `graph.a` and the reach rides on
//! the accessor's name.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const SERVICES: &str = "package p

class ServiceA {
    fun doA(): Int = 1
}

class ServiceB {
    fun doB(): Int = 2
}
";

const GRAPH: &str = "package p

class TestGraph {
    val a: ServiceA = ServiceA()
    val b: ServiceB = ServiceB()

    companion object {
        fun create(): TestGraph = TestGraph()
    }
}

class Ext {
    val graph = TestGraph.create()
}

object Fixtures {
    fun repo(): ServiceA = ServiceA()
}
";

const TESTS: &[(&str, &str, &str)] = &[
    (
        "UsesATest",
        "    private val graph = TestGraph.create()",
        "graph.a.doA()",
    ),
    (
        "UsesBTest",
        "    private val graph = TestGraph.create()",
        "graph.b.doB()",
    ),
    (
        "NestedTest",
        "    private val ext = Ext()",
        "ext.graph.a.doA()",
    ),
    ("FunAccessorTest", "", "Fixtures.repo().doA()"),
    (
        "WithScopeTest",
        "    private val graph = TestGraph.create()",
        "with(graph.a) { doA() }",
    ),
    (
        "LetTest",
        "    private val graph = TestGraph.create()",
        "graph.a.let { it.doA() }",
    ),
    (
        "AlsoTest",
        "    private val graph = TestGraph.create()",
        "graph.a.also { it.doA() }",
    ),
    (
        "DestructureTest",
        "    private val graph = TestGraph.create()",
        "val (x, _) = Pair(graph.a, graph.b)\n        x.doA()",
    ),
];

const A_USERS: &[&str] = &[
    "p.UsesATest",
    "p.NestedTest",
    "p.FunAccessorTest",
    "p.WithScopeTest",
    "p.LetTest",
    "p.AlsoTest",
    "p.DestructureTest",
];

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

fn fixture(tag: &str, config: Option<&str>) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "kt-affected-accessors-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    write(&root, "settings.gradle.kts", "include(\":m\")\n");
    write(&root, "m/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
    if let Some(c) = config {
        write(&root, "kt-affected.toml", c);
    }
    write(&root, "m/src/main/kotlin/p/Services.kt", SERVICES);
    write(&root, "m/src/test/kotlin/p/TestGraph.kt", GRAPH);
    for (name, members, body) in TESTS {
        let text = format!(
            "package p\n\nimport kotlin.test.Test\n\nclass {name} {{\n{members}\n\n    @Test\n    fun t() {{\n        {body}\n    }}\n}}\n"
        );
        write(&root, &format!("m/src/test/kotlin/p/{name}.kt"), &text);
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    root
}

fn select_after(tag: &str, rel: &str, from: &str, to: &str) -> BTreeSet<String> {
    select_with(tag, None, rel, from, to)
}

fn select_with(
    tag: &str,
    config: Option<&str>,
    rel: &str,
    from: &str,
    to: &str,
) -> BTreeSet<String> {
    let root = fixture(tag, config);
    let path = root.join(rel);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{from} not in {rel}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_kt-affected"))
        .args(["select", "--base", "HEAD", "--format", "classes", "--root"])
        .arg(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| l.split('\t').nth(1).map(str::to_string))
        .collect()
}

#[test]
fn a_service_edit_reaches_every_test_that_calls_it_through_an_accessor_and_no_other() {
    let got = select_after(
        "a",
        "m/src/main/kotlin/p/Services.kt",
        "fun doA(): Int = 1",
        "fun doA(): Int = 11",
    );
    for must in A_USERS {
        assert!(got.contains(*must), "{must} missing from {got:?}");
    }
    assert!(
        !got.contains("p.UsesBTest"),
        "UsesBTest never calls doA: {got:?}"
    );
}

#[test]
fn the_other_services_edit_reaches_only_its_own_caller() {
    let got = select_after(
        "b",
        "m/src/main/kotlin/p/Services.kt",
        "fun doB(): Int = 2",
        "fun doB(): Int = 22",
    );
    assert!(got.contains("p.UsesBTest"), "{got:?}");
    for never in A_USERS {
        assert!(!got.contains(*never), "{never} never calls doB: {got:?}");
    }
}

#[test]
fn a_graph_wiring_edit_reaches_every_test_that_uses_the_graph() {
    let got = select_after(
        "wiring",
        "m/src/test/kotlin/p/TestGraph.kt",
        "    val b: ServiceB = ServiceB()\n",
        "    val b: ServiceB = ServiceB()\n    val c: ServiceA = ServiceA()\n",
    );
    for must in ["p.UsesATest", "p.UsesBTest"] {
        assert!(got.contains(must), "{must} missing from {got:?}");
    }
}

#[test]
fn naming_the_graph_type_in_config_keeps_every_accessor_caller() {
    let cfg = "[container_gate]\ngraph_types = [\"TestGraph\"]\n";
    let got = select_with(
        "cfg",
        Some(cfg),
        "m/src/main/kotlin/p/Services.kt",
        "fun doA(): Int = 1",
        "fun doA(): Int = 11",
    );
    for must in A_USERS {
        assert!(got.contains(*must), "{must} missing from {got:?}");
    }
}

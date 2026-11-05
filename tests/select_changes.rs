//! End-to-end: DI-bound implementations dispatch like Koin-provided ones, and a cosmetic edit
//! seeds nothing while a functional one reaches its tests. The real binary against a throwaway
//! git repo; any config lives outside the repo, since one inside is a selector change.

use std::path::{Path, PathBuf};
use std::process::Command;

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

fn repo(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kt-affected-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    write(&root, "settings.gradle.kts", "include(\":m\")\n");
    write(&root, ".gitignore", "build/\n");
    write(&root, "m/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
    for (rel, text) in files {
        write(&root, rel, text);
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    root
}

fn run(root: &Path, cmd: &str, config: Option<&Path>) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_kt-affected"));
    c.args([cmd, "--base", "HEAD", "--root"]).arg(root);
    if cmd == "select" {
        c.args(["--format", "classes"]);
    }
    if let Some(f) = config {
        c.arg("--config").arg(f);
    }
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn picked(root: &Path, config: Option<&Path>) -> Vec<String> {
    run(root, "select", config)
        .lines()
        .filter_map(|l| l.split('\t').nth(1).map(str::to_string))
        .collect()
}

fn test_class(name: &str, members: &str, body: &str) -> String {
    format!(
        "package p\n\nimport kotlin.test.Test\n\nclass {name} {{\n{members}\n\n    @Test\n    fun t() {{\n        {body}\n    }}\n}}\n"
    )
}

// ------------------------------------------------------------------ DI dispatch

const COLORS: &str = "package p

interface Colors {
    fun effective(own: Int?, type: Int?): Int?
}

@ContributesBinding(AppScope::class)
class ColorsImpl : Colors {
    override fun effective(own: Int?, type: Int?): Int? = own ?: type
}
";

/// The impl calls the colour resolver through a typed property, so the edit reaches
/// `EventServiceImpl.getEvent`; only the binding makes callers of `EventService.getEvent` see it.
fn events(binding: &str) -> String {
    format!(
        "package p

interface EventService {{
    fun getEvent(id: Int): Int?
    fun ping(): Int
}}

{binding}class EventServiceImpl(private val colors: Colors) : EventService {{
    override fun getEvent(id: Int): Int? = colors.effective(id, null)
    override fun ping(): Int = 0
}}
"
    )
}

/// A graph that hands out the service through an accessor interface, as a Metro test graph does.
const GRAPH: &str = "package p

interface HasEventService {
    val eventService: EventService
}

@DependencyGraph(AppScope::class)
interface AppGraph : HasEventService

fun appGraph(): AppGraph = TODO()
";

fn di_fixture(tag: &str, binding: &str, extra: &[(&str, &str)]) -> PathBuf {
    let t = "m/src/test/kotlin/p";
    let ev = events(binding);
    let color_test = test_class(
        "ColorTest",
        "    private val graph = appGraph()\n    private val service get() = graph.eventService",
        "service.getEvent(1)",
    );
    let ping_test = test_class(
        "PingTest",
        "    private val graph = appGraph()",
        "graph.eventService.ping()",
    );
    // Holds the graph, but its `getEvent` is an unrelated type's.
    let other_test = test_class(
        "OtherTest",
        "    private val graph = appGraph()\n    private val other = Other()",
        "graph.hashCode()\n        other.getEvent(1)",
    );
    let mut files: Vec<(&str, &str)> = vec![
        ("m/src/main/kotlin/p/Colors.kt", COLORS),
        ("m/src/main/kotlin/p/Events.kt", &ev),
        ("m/src/main/kotlin/p/Graph.kt", GRAPH),
        (
            "m/src/main/kotlin/p/Other.kt",
            "package p\n\nclass Other {\n    fun getEvent(id: Int): Int = id\n}\n",
        ),
    ];
    let (cp, pp, op) = (
        format!("{t}/ColorTest.kt"),
        format!("{t}/PingTest.kt"),
        format!("{t}/OtherTest.kt"),
    );
    files.push((&cp, &color_test));
    files.push((&pp, &ping_test));
    files.push((&op, &other_test));
    files.extend_from_slice(extra);
    let root = repo(tag, &files);
    write(
        &root,
        "m/src/main/kotlin/p/Colors.kt",
        &COLORS.replace("own ?: type", "type ?: own"),
    );
    root
}

#[test]
fn a_contributes_binding_impl_reaches_callers_of_its_interface_through_an_inferred_receiver() {
    let root = di_fixture("di-binding", "@ContributesBinding(AppScope::class)\n", &[]);
    let got = picked(&root, None);
    let cfg = std::env::temp_dir().join(format!("kt-affected-no-di-{}.toml", std::process::id()));
    std::fs::write(
        &cfg,
        "[di]\nbinding_annotations = []\nprovider_annotations = []\n",
    )
    .unwrap();
    let without = picked(&root, Some(&cfg));
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    let _ = std::fs::remove_file(&cfg);
    assert!(
        got.iter().any(|c| c == "p.ColorTest"),
        "ColorTest missing from {got:?}"
    );
    // `ping()` never runs the edited code; `Other.getEvent` is not the interface's.
    assert!(
        !got.iter().any(|c| c == "p.PingTest"),
        "PingTest wrongly in {got:?}"
    );
    assert!(
        !got.iter().any(|c| c == "p.OtherTest"),
        "OtherTest wrongly in {got:?}"
    );
    // Without the binding the edit stops at the impl: the interface's callers never see it.
    assert!(
        !without.iter().any(|c| c == "p.ColorTest"),
        "no DI config still picked {without:?}"
    );
}

#[test]
fn a_provides_bound_impl_dispatches_like_an_annotated_one() {
    let bindings = "package p

@BindingContainer
object Bindings {
    @Provides
    fun events(colors: Colors): EventService = EventServiceImpl(colors)
}
";
    let root = di_fixture(
        "di-provides",
        "",
        &[("m/src/main/kotlin/p/Bindings.kt", bindings)],
    );
    let got = picked(&root, None);
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(
        got.iter().any(|c| c == "p.ColorTest"),
        "ColorTest missing from {got:?}"
    );
    assert!(
        !got.iter().any(|c| c == "p.PingTest"),
        "PingTest wrongly in {got:?}"
    );
}

#[test]
fn explain_prints_the_verdict_and_the_path_with_its_dispatch_edge() {
    let root = di_fixture("di-explain", "@ContributesBinding(AppScope::class)\n", &[]);
    let out = run(&root, "explain", None);
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(out.contains("functional ColorsImpl.effective"), "{out}");
    let path = out.split("p.ColorTest").nth(1).expect("ColorTest path");
    assert!(
        path.contains("ColorsImpl.effective")
            && path
                .contains("-[DI dispatch: called through its interface]-> EventService.getEvent"),
        "{out}"
    );
}

// ------------------------------------------------------------------ functional vs cosmetic

const REPO: &str = "package p

import kotlin.math.abs
import kotlin.math.max

class Repo(private val scale: Int) {
    private val log = mutableListOf<String>()

    fun foo(value: Int, other: Int): Int {
        val doubled = value * 2
        log.add(\"foo called\")
        return listOf(doubled).map { x -> helper(x, this.scale) }.sum() + max(abs(other), 0) + \"ab\".length
    }

    fun extra(): Int = 3

    private fun helper(n: Int, m: Int): Int = n + m
}
";

/// (what the edit is, old, new, functional?)
const EDITS: &[(&str, &str, &str, bool)] = &[
    (
        "reformat",
        "val doubled = value * 2",
        "val doubled =\n            value   *   2",
        false,
    ),
    (
        "comment on a code line",
        "log.add(\"foo called\")",
        "log.add(\"foo called\") // trace",
        false,
    ),
    (
        "KDoc",
        "    fun foo(",
        "    /** Sums things. */\n    fun foo(",
        false,
    ),
    (
        "import reorder",
        "import kotlin.math.abs\nimport kotlin.math.max",
        "import kotlin.math.max\nimport kotlin.math.abs",
        false,
    ),
    (
        "unused import",
        "import kotlin.math.max\n",
        "import kotlin.math.max\nimport kotlin.math.min\n",
        false,
    ),
    (
        "trailing comma",
        "fun foo(value: Int, other: Int): Int {",
        "fun foo(\n        value: Int,\n        other: Int,\n    ): Int {",
        false,
    ),
    (
        "redundant parentheses",
        "val doubled = value * 2",
        "val doubled = (value) * 2",
        false,
    ),
    ("redundant this.", "this.scale", "scale", false),
    (
        "local rename",
        "val doubled = value * 2",
        "val twice = value * 2",
        false,
    ),
    (
        "lambda parameter rename",
        "{ x -> helper(x, this.scale) }",
        "{ y -> helper(y, this.scale) }",
        false,
    ),
    (
        "private parameter rename",
        "helper(n: Int, m: Int): Int = n + m",
        "helper(a: Int, b: Int): Int = a + b",
        false,
    ),
    (
        "@Suppress",
        "    fun foo(",
        "    @Suppress(\"MagicNumber\")\n    fun foo(",
        false,
    ),
    (
        "public parameter rename",
        "fun foo(value: Int, other: Int): Int {\n        val doubled = value * 2",
        "fun foo(input: Int, other: Int): Int {\n        val doubled = input * 2",
        true,
    ),
    ("string literal", "\"ab\".length", "\"abc\".length", true),
    (
        "log message",
        "log.add(\"foo called\")",
        "log.add(\"foo invoked\")",
        true,
    ),
    (
        "declaration removed",
        "    fun extra(): Int = 3\n\n",
        "",
        true,
    ),
    (
        "@JvmName",
        "    fun foo(",
        "    @JvmName(\"foo2\")\n    fun foo(",
        true,
    ),
];

#[test]
fn cosmetic_edits_seed_nothing_and_functional_ones_reach_their_test() {
    let test = test_class("RepoTest", "", "Repo(1).foo(1, 2)");
    let root = repo(
        "changes",
        &[
            ("m/src/main/kotlin/p/Repo.kt", REPO),
            ("m/src/test/kotlin/p/RepoTest.kt", &test),
        ],
    );
    let mut wrong = Vec::new();
    for &(what, old, new, functional) in EDITS {
        assert_eq!(1, REPO.matches(old).count(), "{what}: edit must apply once");
        let mut edited = REPO.replace(old, new);
        if what == "local rename" {
            edited = edited.replace("listOf(doubled)", "listOf(twice)");
        }
        write(&root, "m/src/main/kotlin/p/Repo.kt", &edited);
        let got = picked(&root, None);
        if got.iter().any(|c| c == "p.RepoTest") != functional {
            wrong.push(format!(
                "{what}: picked {got:?}\n{}",
                run(&root, "explain", None)
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

#[test]
fn classification_off_seeds_every_touched_declaration() {
    let test = test_class("RepoTest", "", "Repo(1).foo(1, 2)");
    let root = repo(
        "classify-off",
        &[
            ("m/src/main/kotlin/p/Repo.kt", REPO),
            ("m/src/test/kotlin/p/RepoTest.kt", &test),
        ],
    );
    write(
        &root,
        "m/src/main/kotlin/p/Repo.kt",
        &REPO.replace("val doubled = value * 2", "val doubled = (value) * 2"),
    );
    let cfg = std::env::temp_dir().join(format!(
        "kt-affected-no-classify-{}.toml",
        std::process::id()
    ));
    std::fs::write(&cfg, "[changes]\nclassify = false\n").unwrap();
    let got = picked(&root, Some(&cfg));
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    let _ = std::fs::remove_file(&cfg);
    assert!(got.iter().any(|c| c == "p.RepoTest"), "{got:?}");
}

#[test]
fn a_declaration_a_changed_resource_names_still_seeds_though_its_own_text_is_unchanged() {
    let loader = "package p\n\nclass Loader {\n    fun load(): String = read(\"items.json\")\n\n    private fun read(name: String): String = name\n}\n";
    let test = test_class("LoaderTest", "", "Loader().load()");
    let root = repo(
        "mention",
        &[
            ("config/items.json", "{}\n"),
            ("m/src/main/kotlin/p/Loader.kt", loader),
            ("m/src/test/kotlin/p/LoaderTest.kt", &test),
        ],
    );
    write(&root, "config/items.json", "{\"a\": 1}\n");
    let got = picked(&root, None);
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    // Loader.kt is unchanged, so its declarations compare equal; the mention must seed them anyway.
    assert!(got.iter().any(|c| c == "p.LoaderTest"), "{got:?}");
}

#[test]
fn a_test_whose_graph_excludes_the_impl_is_dropped_and_one_on_the_production_binding_kept() {
    // The fake graph leaves EventServiceImpl out and binds a fake: its tests can never run the edit.
    let fake_graph = "package p

@DependencyGraph(AppScope::class, excludes = [EventServiceImpl::class])
interface FakeGraph : HasEventService {
    @DependencyGraph.Factory
    fun interface Factory {
        fun create(@Includes fakes: FakeBindings): FakeGraph
    }
}

@BindingContainer
object FakeBindings {
    @Provides
    fun events(): EventService = object : EventService {
        override fun getEvent(id: Int): Int? = 0
        override fun ping(): Int = 0
    }
}

fun fakeGraph(): FakeGraph = TODO()
";
    let fake_test = test_class(
        "FakeGraphTest",
        "    private val graph = fakeGraph()\n    private val service get() = graph.eventService",
        "service.getEvent(1)",
    );
    let root = di_fixture(
        "di-excludes",
        "@ContributesBinding(AppScope::class)\n",
        &[
            ("m/src/test/kotlin/p/FakeGraph.kt", fake_graph),
            ("m/src/test/kotlin/p/FakeGraphTest.kt", &fake_test),
        ],
    );
    let got = picked(&root, None);
    let out = run(&root, "explain", None);
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(
        got.iter().any(|c| c == "p.ColorTest"),
        "production binding dropped: {got:?}\n{out}"
    );
    assert!(
        !got.iter().any(|c| c == "p.FakeGraphTest"),
        "excluding graph kept: {got:?}\n{out}"
    );
}

#[test]
fn a_test_whose_classpath_lacks_the_impl_module_never_sees_it_through_the_interface() {
    let root = std::env::temp_dir().join(format!("kt-affected-classpath-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    write(&root, ".gitignore", "build/\n");
    write(
        &root,
        "settings.gradle.kts",
        "include(\":api\", \":impl\", \":apionly\", \":wired\")\n",
    );
    write(
        &root,
        "api/build.gradle.kts",
        "plugins { kotlin(\"jvm\") }\n",
    );
    write(
        &root,
        "impl/build.gradle.kts",
        "plugins { kotlin(\"jvm\") }\ndependencies { implementation(project(\":api\")) }\n",
    );
    write(
        &root,
        "apionly/build.gradle.kts",
        "plugins { kotlin(\"jvm\") }\ndependencies { implementation(project(\":api\")) }\n",
    );
    write(
        &root,
        "wired/build.gradle.kts",
        "plugins { kotlin(\"jvm\") }\ndependencies { implementation(project(\":api\"))\n    testImplementation(project(\":impl\")) }\n",
    );
    write(
        &root,
        "api/src/main/kotlin/p/Api.kt",
        "package p\n\ninterface EventService {\n    fun getEvent(id: Int): Int?\n}\n\ninterface HasEventService {\n    val eventService: EventService\n}\n",
    );
    write(
        &root,
        "impl/src/main/kotlin/p/Impl.kt",
        "package p\n\n@ContributesBinding(AppScope::class)\nclass EventServiceImpl : EventService {\n    override fun getEvent(id: Int): Int? = helper(id)\n\n    fun helper(id: Int): Int? = id\n}\n",
    );
    for (m, g) in [("apionly", "ApiOnlyGraph"), ("wired", "WiredGraph")] {
        write(
            &root,
            &format!("{m}/src/test/kotlin/p/{g}.kt"),
            &format!(
                "package p\n\n@DependencyGraph(AppScope::class)\ninterface {g} : HasEventService\n\nfun make{g}(): {g} = TODO()\n"
            ),
        );
        let t = format!("{g}Test");
        write(
            &root,
            &format!("{m}/src/test/kotlin/p/{t}.kt"),
            &test_class(
                &t,
                &format!(
                    "    private val graph = make{g}()\n    private val service get() = graph.eventService"
                ),
                "service.getEvent(1)",
            ),
        );
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    write(
        &root,
        "impl/src/main/kotlin/p/Impl.kt",
        "package p\n\n@ContributesBinding(AppScope::class)\nclass EventServiceImpl : EventService {\n    override fun getEvent(id: Int): Int? = helper(id)\n\n    fun helper(id: Int): Int? = id + 1\n}\n",
    );
    let got = picked(&root, None);
    let out = run(&root, "explain", None);
    let _ = std::fs::remove_dir_all(&root); // best effort: Windows may still hold a handle
    assert!(
        got.iter().any(|c| c == "p.WiredGraphTest"),
        "impl on the test classpath dropped: {got:?}\n{out}"
    );
    assert!(
        !got.iter().any(|c| c == "p.ApiOnlyGraphTest"),
        "api-only module kept: {got:?}\n{out}"
    );
}

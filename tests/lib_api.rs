//! The library API: `select` and `explain` on a throwaway git repo, an unanswerable case as an
//! `Err`, and a `Selection` surviving a JSON round trip.

use kt_affected::{Error, SelectOptions, Selection};
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

fn tmp(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kt-affected-lib-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

const BOOK_TEST: &str = "package p

import kotlin.test.Test

class BookTest {
    @Test
    fun readsTheTitle() {
        Book(\"Dune\").title()
    }
}
";

const AUTHOR_TEST: &str = "package p

import kotlin.test.Test

class AuthorTest {
    @Test
    fun readsTheName() {
        Author(\"Herbert\").name()
    }
}
";

/// Two classes with a test each, committed; then a body edit to `Book`, committed.
fn library(tag: &str) -> PathBuf {
    let root = tmp(tag);
    write(&root, "settings.gradle.kts", "include(\":m\")\n");
    write(&root, ".gitignore", "build/\n");
    write(&root, "m/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n");
    write(
        &root,
        "m/src/main/kotlin/p/Book.kt",
        "package p\n\nclass Book(val name: String) {\n    fun title(): String = name\n}\n",
    );
    write(
        &root,
        "m/src/main/kotlin/p/Author.kt",
        "package p\n\nclass Author(val n: String) {\n    fun name(): String = n\n}\n",
    );
    write(&root, "m/src/test/kotlin/p/BookTest.kt", BOOK_TEST);
    write(&root, "m/src/test/kotlin/p/AuthorTest.kt", AUTHOR_TEST);
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    write(
        &root,
        "m/src/main/kotlin/p/Book.kt",
        "package p\n\nclass Book(val name: String) {\n    fun title(): String = name.uppercase()\n}\n",
    );
    git(&root, &["commit", "-qam", "edit"]);
    root
}

#[test]
fn select_returns_the_tests_that_reach_the_edit() {
    let root = library("select");
    let opts = SelectOptions::new().base("HEAD~1").head("HEAD");
    let sel = kt_affected::select(&root, &opts).unwrap();
    assert_eq!(vec!["m/src/main/kotlin/p/Book.kt"], sel.changed_files);
    assert_eq!(vec![":m:test\tp.BookTest"], sel.to_classes());
    assert_eq!(
        vec![":m:test", "--tests", "p.BookTest"],
        sel.to_gradle_args()
    );
    assert_eq!(None, sel.error);
    // A second call reads the cache the first one wrote, and answers the same.
    assert_eq!(sel, kt_affected::select(&root, &opts).unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_directory_inside_the_build_resolves_to_its_root() {
    let root = library("nested");
    let opts = SelectOptions::new().base("HEAD~1").head("HEAD");
    let sel = kt_affected::select(&root.join("m/src"), &opts).unwrap();
    assert_eq!(vec![":m:test\tp.BookTest"], sel.to_classes());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn explain_gives_the_verdict_and_the_path() {
    let root = library("explain");
    let opts = SelectOptions::new().base("HEAD~1").head("HEAD");
    let e = kt_affected::explain(&root, &opts).unwrap();
    assert_eq!(vec![":m:test\tp.BookTest"], e.selection.to_classes());
    assert!(
        e.verdicts
            .iter()
            .any(|v| v.functional && v.file == "m/src/main/kotlin/p/Book.kt"),
        "{e}"
    );
    let path = e.paths.iter().find(|p| p.test == "p.BookTest").unwrap();
    assert!(
        path.steps.last().unwrap().declaration.contains("title"),
        "{e}"
    );
    assert!(
        e.render()
            .contains("selected tests (1 classes in 1 groups):")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_settings_file_is_an_error_not_an_empty_selection() {
    let root = tmp("no-build");
    write(&root, "src/A.kt", "class A\n");
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    let opts = SelectOptions::new().base("HEAD");
    match kt_affected::select(&root, &opts) {
        Err(Error::NoBuildRoot(dir)) => assert_eq!(root, dir),
        other => panic!("expected NoBuildRoot, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_unknown_base_is_a_git_error() {
    let root = library("bad-base");
    let opts = SelectOptions::new().base("no-such-ref");
    assert!(matches!(
        kt_affected::select(&root, &opts),
        Err(Error::Git(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_selection_survives_a_json_round_trip() {
    let root = library("serde");
    let opts = SelectOptions::new().base("HEAD~1").head("HEAD");
    let sel = kt_affected::select(&root, &opts).unwrap();
    let json = serde_json::to_string_pretty(&sel).unwrap();
    assert!(json.contains("\"changedFiles\""), "{json}");
    assert!(json.contains("\"failOpenForks\""), "{json}");
    let back: Selection = serde_json::from_str(&json).unwrap();
    assert_eq!(sel, back);
    let e = kt_affected::explain(&root, &opts).unwrap();
    let back: kt_affected::Explanation =
        serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
    assert_eq!(e, back);
    let _ = std::fs::remove_dir_all(&root);
}

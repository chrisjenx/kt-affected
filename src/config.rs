//! `kt-affected.toml`: the project-specific knobs selection reads. Every field has a generic
//! default, so a repo with no config file still gets a sound selection.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const CONFIG_FILE: &str = "kt-affected.toml";

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub selector: Selector,
    pub database: Database,
    pub modules: ModulePolicy,
    pub tasks: Tasks,
    #[serde(rename = "source_scanner")]
    pub source_scanners: Vec<SourceScanner>,
    /// Typed dispatch: a subclass of a base reaches what names its type argument, not every caller
    /// of the base's members.
    #[serde(rename = "dispatch")]
    pub dispatch: Vec<Dispatch>,
    /// Name-gated container walk: see [ContainerGate].
    pub container_gate: ContainerGate,
    /// DI-bound implementations: see [Di].
    pub di: Di,
    /// Change classification: see [Changes].
    pub changes: Changes,
    /// What `unreferenced` treats as called from outside the code: see [Unreferenced].
    pub unreferenced: Unreferenced,
}

/// Declarations something outside the Kotlin sources calls (a test runner, a framework, reflection,
/// generated code), so no edge reaches them: `unreferenced` never reports these.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Unreferenced {
    /// Annotations (simple names) marking an entry point. On a class, its constructor properties
    /// are entries too.
    pub entry_annotations: Vec<String>,
    /// Simple names that are entry points wherever they are declared.
    pub entry_names: Vec<String>,
    /// Globs (`*` any run of characters) over the simple or fully qualified name: never reported.
    pub ignored_names: Vec<String>,
    /// Supertypes (simple names, as written, through in-repo supertypes too) whose subtypes a
    /// framework instantiates: those types are entries.
    pub entry_supertypes: Vec<String>,
}

impl Default for Unreferenced {
    fn default() -> Self {
        Unreferenced {
            entry_annotations: [
                // test runners
                "Test",
                "ParameterizedTest",
                "RepeatedTest",
                "TestFactory",
                "TestTemplate",
                "BeforeTest",
                "AfterTest",
                "BeforeEach",
                "AfterEach",
                "BeforeAll",
                "AfterAll",
                "Before",
                "After",
                "BeforeClass",
                "AfterClass",
                "Rule",
                "ClassRule",
                "RegisterExtension",
                "RunWith",
                "ExtendWith",
                "Parameters",
                // tooling, generated code and reflection
                "Preview",
                "Provides",
                "Binds",
                "BindsInstance",
                "IntoSet",
                "IntoMap",
                "ElementsIntoSet",
                "Multibinds",
                "ContributesTo",
                "BindingContainer",
                "HiltAndroidApp",
                "AndroidEntryPoint",
                "Serializable",
                "JsExport",
                "Keep",
            ]
            .map(String::from)
            .to_vec(),
            entry_names: vec!["main".into()],
            ignored_names: vec![],
            entry_supertypes: vec![],
        }
    }
}

/// A class a DI framework binds to its interface is called through that interface: callers of the
/// interface member see a change to the implementation, as with a Koin-provided class.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Di {
    /// Class annotations that bind the class (simple names).
    pub binding_annotations: Vec<String>,
    /// Function annotations whose body binds what it references (`@Provides`, `@Binds`).
    pub provider_annotations: Vec<String>,
    /// How many times a bound service reached through its interface dispatches on.
    pub dispatch_rounds: usize,
    /// "typed": a dispatch round reaches callers whose call resolves to the interface member, and
    /// callers that read a holder of the interface and write the member's name. "names": the
    /// unnamed-receiver step's container walk as well, which keeps any code in a test class that
    /// writes the name.
    pub dispatch: String,
}

impl Default for Di {
    fn default() -> Self {
        Di {
            binding_annotations: [
                "ContributesBinding",
                "ContributesIntoSet",
                "ContributesIntoMap",
                "ContributesMultibinding",
                "Inject",
                "AssistedInject",
                "HiltViewModel",
            ]
            .map(String::from)
            .to_vec(),
            provider_annotations: ["Provides", "Binds"].map(String::from).to_vec(),
            dispatch_rounds: 64,
            dispatch: "typed".into(),
        }
    }
}

/// A changed declaration whose old and new syntax trees agree once cosmetic differences are
/// normalised away seeds nothing. Off: every touched declaration seeds.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Changes {
    pub classify: bool,
    /// Annotations whose addition, removal or arguments never change behaviour (simple names).
    pub cosmetic_annotations: Vec<String>,
}

impl Default for Changes {
    fn default() -> Self {
        Changes {
            classify: true,
            cosmetic_annotations: vec!["Suppress".into()],
        }
    }
}

/// A member edit's walk from its class to every member of the class's supertypes keeps only what can
/// observe the edit: declarations that write a changed member's name. Off for whole-type,
/// construction-time and implicitly called changes, and for changes inside a DI graph, whose wiring
/// every user of the graph observes.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ContainerGate {
    pub enabled: bool,
    /// Annotations marking a DI graph type (simple names).
    pub graph_annotations: Vec<String>,
    /// Further graph or test-extension types by simple name, for graphs no annotation marks.
    pub graph_types: Vec<String>,
}

impl Default for ContainerGate {
    fn default() -> Self {
        ContainerGate {
            enabled: true,
            graph_annotations: [
                "DependencyGraph",
                "GraphExtension",
                "Component",
                "Subcomponent",
                "MergeComponent",
            ]
            .map(String::from)
            .to_vec(),
            graph_types: Vec::new(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Selector {
    /// A change to one of these fails every module open (the selector's own rules).
    pub self_paths: Vec<String>,
    /// Paths that cannot change a test outcome unless a test names them.
    pub ignored_prefixes: Vec<String>,
    pub doc_suffixes: Vec<String>,
    /// Ignorable only outside every module (inside one, a test may read it as a fixture).
    pub outside_module_ignored_suffixes: Vec<String>,
    pub module_ignored_basenames: Vec<String>,
    /// Extra build-logic sources (beyond build scripts and included builds) whose text decides
    /// whether a file outside the modules is a build input.
    pub build_logic_globs: Vec<String>,
    /// A test the graph walk reaches is kept only if an edited module is in its module's dependency
    /// closure: code not on a test's classpath never runs in it.
    pub classpath_bound: bool,
    /// Gradle plugin ids and Maven groups (prefixes) of compiler plugins and code generators: a
    /// version-catalog change to one changes the code every module compiles, so it runs every class.
    pub codegen_plugins: Vec<String>,
}

impl Default for Selector {
    fn default() -> Self {
        Selector {
            self_paths: vec![],
            ignored_prefixes: vec![
                "docs/".into(),
                ".idea/".into(),
                ".github/".into(),
                ".run/".into(),
            ],
            doc_suffixes: vec![".md".into()],
            outside_module_ignored_suffixes: vec![".txt".into()],
            module_ignored_basenames: vec!["detekt-baseline.xml".into()],
            build_logic_globs: vec![],
            classpath_bound: true,
            codegen_plugins: [
                "org.jetbrains.kotlin",
                "org.jetbrains.compose",
                "com.google.devtools.ksp",
                "dev.zacsweers.metro",
                "app.cash.sqldelight",
                "com.squareup.anvil",
                "com.google.dagger",
                "androidx.room",
                "com.apollographql.apollo",
                "com.squareup.wire",
                "com.google.protobuf",
                "org.jetbrains.kotlinx.atomicfu",
                "dev.drewhamilton.poko",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// Flyway-style migration directories (path prefixes).
    pub migration_dirs: Vec<String>,
    /// A module whose build script contains this text runs a database in its tests.
    pub module_marker: Option<String>,
    /// Files a build plugin copies onto the database modules' test classpath.
    pub synced_inputs: Vec<String>,
    /// A database-backed test class names one of these.
    pub test_markers: Vec<String>,
    /// A Kotlin file must contain one of these to count as SQL-touching code upstream of a test.
    pub sql_file_markers: Vec<String>,
}

impl Default for Database {
    fn default() -> Self {
        Database {
            migration_dirs: vec![],
            module_marker: None,
            synced_inputs: vec![],
            test_markers: vec![],
            sql_file_markers: vec![
                "org.jetbrains.exposed".into(),
                "java.sql.".into(),
                "javax.sql.".into(),
            ],
        }
    }
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ModulePolicy {
    /// Runs only for direct changes under the module's directory.
    pub direct_only: BTreeSet<String>,
    /// Its fail-open run needs parallel forks.
    pub fail_open_forks: BTreeSet<String>,
    /// Never runs from the selector.
    pub manual: BTreeSet<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Tasks {
    /// source set -> Gradle test tasks, replacing what discovery derived for it.
    pub source_sets: BTreeMap<String, Vec<String>>,
    /// Tasks run in the local loop; when empty, JVM-hosted tasks are local.
    pub local: BTreeSet<String>,
    /// Only these tasks are emitted for a source set (when non-empty). Lets a project keep the
    /// one-task-per-source-set shape of an older selector.
    pub only: BTreeSet<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourceScanner {
    /// A changed path matching this regex (and not `exclude_path`) selects the scanner's tests.
    pub changed_path: String,
    /// A changed path matching this regex never triggers the scanner, though `changed_path`
    /// matches it (the `regex` crate has no lookahead to say "outside these roots").
    #[serde(default)]
    pub exclude_path: Option<String>,
    /// Every test class in this package and its subpackages. Exclusive with `test_classes`.
    #[serde(default)]
    pub test_package: Option<String>,
    /// These fully qualified test classes only. Exclusive with `test_package`.
    #[serde(default)]
    pub test_classes: Option<Vec<String>>,
}

impl SourceScanner {
    fn validate(&self, i: usize) -> Result<(), String> {
        let at = format!("[[source_scanner]] #{}", i + 1);
        match (&self.test_package, &self.test_classes) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "{at}: set `test_package` or `test_classes`, not both"
                ));
            }
            (None, None) => return Err(format!("{at}: set `test_package` or `test_classes`")),
            (None, Some(c)) if c.is_empty() => {
                return Err(format!("{at}: `test_classes` is empty"));
            }
            _ => {}
        }
        for (key, re) in [
            ("changed_path", Some(&self.changed_path)),
            ("exclude_path", self.exclude_path.as_ref()),
        ] {
            if let Some(re) = re {
                regex::Regex::new(re).map_err(|e| format!("{at}: `{key}`: {e}"))?;
            }
        }
        Ok(())
    }

    /// Does a change to `path` trigger this scanner? (`changed` and `exclude` are its compiled
    /// `changed_path` and `exclude_path`.)
    pub fn triggered_by(
        &self,
        changed: &regex::Regex,
        exclude: Option<&regex::Regex>,
        path: &str,
    ) -> bool {
        changed.is_match(path) && !exclude.is_some_and(|x| x.is_match(path))
    }

    /// Is the test class `fq`, in package `pkg`, one this scanner runs?
    pub fn runs(&self, pkg: &str, fq: &str) -> bool {
        if let Some(c) = &self.test_classes {
            return c.iter().any(|c| c == fq);
        }
        let Some(want) = &self.test_package else {
            return false;
        };
        pkg == want
            || (pkg.starts_with(want.as_str()) && pkg.as_bytes().get(want.len()) == Some(&b'.'))
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    /// Base classes dispatched by their type argument (`Handler<E>` runs when an E is sent).
    pub bases: Vec<String>,
    /// What constructing a subclass runs keeps one database-backed smoke class per module in reach.
    #[serde(default)]
    pub construction_smoke: bool,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config, String> {
        let p = root.join(CONFIG_FILE);
        if p.exists() {
            Config::load_file(&p)
        } else {
            Ok(Config::default())
        }
    }

    pub fn load_file(p: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        Config::parse(&text).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Parse and validate config text.
    pub fn parse(text: &str) -> Result<Config, String> {
        let c: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        for (i, sc) in c.source_scanners.iter().enumerate() {
            sc.validate(i)?;
        }
        Ok(c)
    }

    pub fn is_local_task(&self, task: &str) -> bool {
        if !self.tasks.local.is_empty() {
            return self.tasks.local.contains(task);
        }
        // JVM-hosted by default; browsers, simulators and devices are not.
        !(task.contains("Browser")
            || task.contains("Simulator")
            || task.contains("Device")
            || task.starts_with("ios")
            || task.starts_with("macos")
            || task.starts_with("js")
            || task.starts_with("wasm")
            || task.contains("Node"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_generic_defaults() {
        let d = std::env::temp_dir().join("kt-affected-noconfig");
        let _ = std::fs::create_dir_all(&d);
        let c = Config::load(&d).unwrap();
        assert!(c.selector.self_paths.is_empty());
        assert!(c.is_local_task("jvmTest"));
        assert!(!c.is_local_task("wasmJsBrowserTest"));
    }

    #[test]
    fn parses_every_section() {
        let c: Config = toml::from_str(
            r#"
            [selector]
            self_paths = ["tools/x.py"]
            [database]
            migration_dirs = ["db/migration/"]
            module_marker = "useDb()"
            [modules]
            manual = [":m"]
            [tasks.source_sets]
            mockedUiTest = ["jvmTest", "wasmJsBrowserTest"]
            [[source_scanner]]
            changed_path = "^backend/.*\\.kt$"
            test_package = "co.x.guards"
            [[dispatch]]
            bases = ["Handler"]
            construction_smoke = true
            "#,
        )
        .unwrap();
        assert_eq!(c.selector.self_paths, vec!["tools/x.py"]);
        assert_eq!(c.tasks.source_sets["mockedUiTest"].len(), 2);
        assert!(c.dispatch[0].construction_smoke);
        assert!(c.modules.manual.contains(":m"));
    }

    #[test]
    fn a_source_scanner_names_a_package_or_classes_never_both() {
        let both = r#"
            [[source_scanner]]
            changed_path = "^a/"
            test_package = "x"
            test_classes = ["x.A"]
        "#;
        assert!(Config::parse(both).unwrap_err().contains("not both"));
        assert!(Config::parse("[[source_scanner]]\nchanged_path = '^a/'\n").is_err());
        let bad =
            "[[source_scanner]]\nchanged_path = 'a'\nexclude_path = '('\ntest_package = 'x'\n";
        assert!(Config::parse(bad).unwrap_err().contains("exclude_path"));
        let c = Config::parse(
            "[[source_scanner]]\nchanged_path = '\\.kt$'\nexclude_path = '^backend'\ntest_classes = ['x.A']\n",
        )
        .unwrap();
        let sc = &c.source_scanners[0];
        assert!(sc.runs("x", "x.A") && !sc.runs("x", "x.B"));
        let re = regex::Regex::new(&sc.changed_path).unwrap();
        let ex = regex::Regex::new(sc.exclude_path.as_ref().unwrap()).unwrap();
        assert!(sc.triggered_by(&re, Some(&ex), "app/A.kt"));
        assert!(!sc.triggered_by(&re, Some(&ex), "backend-x/A.kt"));
        let pkg = Config::parse("[[source_scanner]]\nchanged_path = 'a'\ntest_package = 'co.g'\n")
            .unwrap();
        assert!(pkg.source_scanners[0].runs("co.g.sub", "co.g.sub.T"));
        assert!(!pkg.source_scanners[0].runs("co.gx", "co.gx.T"));
    }
}

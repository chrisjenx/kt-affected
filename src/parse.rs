//! Declarations, imports and name tokens of one Kotlin file, from kt-syntax's PSI-shaped tree.
//!
//! The declaration model: one node per class / interface / object / fun / val / var / typealias / enum entry / init block /
//! secondary constructor that a container (file, type, enum entry) declares. Anything declared
//! inside a body folds back into the body that owns it. A node's tokens are every identifier-like
//! word of code (not comments, not string text) inside it and outside its child nodes.

use crate::koin::{self, KoinDef};
use kt_syntax::{Node, SyntaxKind as K, WalkEvent};
use regex::Regex;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::sync::LazyLock;

pub const KIND_CLASS: u8 = 0;
pub const KIND_INTERFACE: u8 = 1;
pub const KIND_OBJECT: u8 = 2;
pub const KIND_FUN: u8 = 3;
pub const KIND_VAL: u8 = 4;
pub const KIND_TYPEALIAS: u8 = 5;
pub const KIND_ENUM_ENTRY: u8 = 6;
pub const KIND_VAR: u8 = 7;
pub const KIND_INIT: u8 = 8;
pub const KIND_CONSTRUCTOR: u8 = 9;

pub const FLAG_PRIVATE: u8 = 1;
pub const FLAG_OVERRIDE: u8 = 2;
/// A top-level declaration the test runner can be handed (`--tests Fqcn`).
pub const FLAG_TEST_ROOT: u8 = 4;
pub const FLAG_INNER: u8 = 8;
/// `data` / `value` / `@Serializable`: equals/hashCode/toString are the type's own.
pub const FLAG_ANY_SEMANTICS: u8 = 16;
pub const FLAG_SEALED: u8 = 32;
/// A primary-constructor property: a name-only span whose type stays in the class header.
pub const FLAG_CTOR_PROP: u8 = 64;
/// A property with an initializer or delegate, which runs on construction.
pub const FLAG_INITIALIZED: u8 = 128;

pub fn is_type_kind(k: u8) -> bool {
    matches!(k, KIND_CLASS | KIND_INTERFACE | KIND_OBJECT)
}

/// class / interface / object / enum entry: what may declare members.
pub fn is_container_kind(k: u8) -> bool {
    matches!(
        k,
        KIND_CLASS | KIND_INTERFACE | KIND_OBJECT | KIND_ENUM_ENTRY
    )
}

/// What a subtype inherits from a type: its members, not its nested types.
pub fn is_inheritable_kind(k: u8) -> bool {
    matches!(
        k,
        KIND_FUN | KIND_VAL | KIND_VAR | KIND_INIT | KIND_CONSTRUCTOR | KIND_ENUM_ENTRY
    )
}

/// HARD keywords only: a token matching one can never be a reference.
pub const KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

pub const MODIFIERS: &[&str] = &[
    "public",
    "internal",
    "private",
    "protected",
    "abstract",
    "open",
    "sealed",
    "data",
    "value",
    "inline",
    "expect",
    "actual",
    "external",
    "suspend",
    "const",
    "lateinit",
    "override",
    "operator",
    "infix",
    "tailrec",
    "annotation",
    "enum",
    "companion",
    "vararg",
    "noinline",
    "crossinline",
    "final",
    "inner",
    "reified",
    "fun",
];

/// One declaration with its strings not yet interned.
#[derive(Default, Debug)]
pub struct RawDecl {
    pub name: String,
    pub kind: u8,
    pub flags: u8,
    pub parent: i32,
    pub lo: u32,
    pub hi: u32,
    pub supers: Vec<String>,
    pub super_args: Vec<String>,
    pub annotations: Vec<String>,
    pub refs: Vec<String>,
    /// First supertype of each `object : X` expression in the declaration's own code.
    pub anon: Vec<String>,
    /// Receiver type path of a top-level extension `fun`/`val`/`var`: `Outer.Inner.f` -> [Outer, Inner].
    pub receiver: Vec<String>,
}

#[derive(Default, Debug)]
pub struct RawImport {
    pub segs: Vec<String>,
    pub alias: Option<String>,
    pub star: bool,
}

#[derive(Default, Debug)]
pub struct RawFile {
    pub package: String,
    pub imports: Vec<RawImport>,
    pub decls: Vec<RawDecl>,
    /// Simple names of non-star imports the code writes anywhere outside the import list.
    pub in_code: Vec<String>,
    /// (declaration, definitions, the module's text outside them) per Koin module.
    pub koin: Vec<(usize, Vec<KoinDef>, KoinDef)>,
    /// (class, what its construction runs; None when the header does not parse)
    pub builds: Vec<(usize, Option<Vec<String>>)>,
    /// (class, names in its primary-constructor parameter types), non-empty only
    pub params: Vec<(usize, Vec<String>)>,
    pub error: bool,
}

pub const NONE: u32 = u32::MAX;

/// A declaration with interned strings.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Decl {
    pub name: u32,
    pub kind: u8,
    pub flags: u8,
    pub parent: i32,
    pub lo: u32,
    pub hi: u32,
    pub supers: Vec<u32>,
    pub super_args: Vec<u32>,
    pub annotations: Vec<u32>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Import {
    pub segs: Vec<u32>,
    /// `prefixes[k]` = interned `segs[..=k]` joined with dots: the package candidates resolution tries.
    pub prefixes: Vec<u32>,
    pub alias: u32,
    pub star: bool,
}

/// What linking one file needs beyond the repo-wide symbol table.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct FileRefs {
    pub imports: Vec<Import>,
    pub refs: Vec<Vec<u32>>,
    pub anon: Vec<Vec<u32>>,
    pub receiver: Vec<Vec<u32>>,
    pub in_code: Vec<u32>,
    pub koin: Vec<KoinModuleI>,
    /// (local class, construction names; None when the header does not parse)
    pub builds: Vec<(u32, Option<Vec<u32>>)>,
    pub params: Vec<(u32, Vec<u32>)>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct KoinDefI {
    pub keys: Vec<u32>,
    pub names: Vec<u32>,
    pub eager: bool,
    pub dotted: Vec<u32>,
    pub qualified: Vec<u32>,
    pub receivers: bool,
    pub untyped: bool,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct KoinModuleI {
    pub decl: u32,
    pub defs: Vec<KoinDefI>,
    pub rest: KoinDefI,
}

#[derive(Clone, Default, Debug)]
pub struct FileParse {
    pub package: u32,
    pub decls: Vec<Decl>,
    pub refs: FileRefs,
    pub error: bool,
}

#[derive(Default)]
pub struct Interner {
    map: FxHashMap<Box<str>, u32>,
    pub strs: Vec<Box<str>>,
}

impl Interner {
    pub fn from_strings(strs: Vec<Box<str>>) -> Self {
        let map = strs
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i as u32))
            .collect();
        Interner { map, strs }
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.map.get(s) {
            return i;
        }
        let i = self.strs.len() as u32;
        let b: Box<str> = s.into();
        self.strs.push(b.clone());
        self.map.insert(b, i);
        i
    }

    pub fn get(&self, s: &str) -> Option<u32> {
        self.map.get(s).copied()
    }

    pub fn intern_file(&mut self, raw: RawFile) -> FileParse {
        let package = self.intern(&raw.package);
        let imports = raw
            .imports
            .into_iter()
            .map(|im| {
                let mut prefixes = Vec::with_capacity(im.segs.len());
                let mut joined = String::new();
                for (k, s) in im.segs.iter().enumerate() {
                    if k > 0 {
                        joined.push('.');
                    }
                    joined.push_str(s);
                    prefixes.push(self.intern(&joined));
                }
                Import {
                    segs: im.segs.iter().map(|s| self.intern(s)).collect(),
                    prefixes,
                    alias: im.alias.as_deref().map(|a| self.intern(a)).unwrap_or(NONE),
                    star: im.star,
                }
            })
            .collect();
        let mut decls = Vec::with_capacity(raw.decls.len());
        let mut refs = Vec::with_capacity(raw.decls.len());
        let mut anon = Vec::with_capacity(raw.decls.len());
        let mut receiver = Vec::with_capacity(raw.decls.len());
        for d in raw.decls {
            refs.push(d.refs.iter().map(|s| self.intern(s)).collect());
            anon.push(d.anon.iter().map(|s| self.intern(s)).collect());
            receiver.push(d.receiver.iter().map(|s| self.intern(s)).collect());
            decls.push(Decl {
                name: self.intern(&d.name),
                kind: d.kind,
                flags: d.flags,
                parent: d.parent,
                lo: d.lo,
                hi: d.hi,
                supers: d.supers.iter().map(|s| self.intern(s)).collect(),
                super_args: d.super_args.iter().map(|s| self.intern(s)).collect(),
                annotations: d.annotations.iter().map(|s| self.intern(s)).collect(),
            });
        }
        let in_code = raw.in_code.iter().map(|s| self.intern(s)).collect();
        let mut koin = Vec::new();
        for (decl, defs, rest) in raw.koin {
            let defs = defs.iter().map(|d| self.intern_def(d)).collect();
            let rest = self.intern_def(&rest);
            koin.push(KoinModuleI {
                decl: decl as u32,
                defs,
                rest,
            });
        }
        let builds = raw
            .builds
            .into_iter()
            .map(|(i, names)| {
                (
                    i as u32,
                    names.map(|v| v.iter().map(|s| self.intern(s)).collect()),
                )
            })
            .collect();
        let params = raw
            .params
            .into_iter()
            .map(|(i, v)| (i as u32, v.iter().map(|s| self.intern(s)).collect()))
            .collect();
        FileParse {
            package,
            decls,
            refs: FileRefs {
                imports,
                refs,
                anon,
                receiver,
                in_code,
                koin,
                builds,
                params,
            },
            error: raw.error,
        }
    }

    fn intern_def(&mut self, d: &KoinDef) -> KoinDefI {
        KoinDefI {
            keys: d.keys.iter().map(|s| self.intern(s)).collect(),
            names: d.names.iter().map(|s| self.intern(s)).collect(),
            eager: d.eager,
            dotted: d.dotted.iter().map(|s| self.intern(s)).collect(),
            // `Owner.name` as (owner, name) pairs.
            qualified: d
                .qualified
                .iter()
                .flat_map(|q| {
                    let (o, n) = q.split_once('.').unwrap_or(("", q));
                    [self.intern(o), self.intern(n)]
                })
                .collect(),
            receivers: d.receivers,
            untyped: d.untyped,
        }
    }
}

static IDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap());
static TOP_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
    let modifier = r"(?:public|internal|private|protected|abstract|open|sealed|data|value|inline|expect|actual|external|suspend|const|lateinit|override|operator|infix|tailrec|annotation|enum|companion|fun|vararg|noinline|crossinline|final)";
    Regex::new(&format!(
        r"(?m)^(?P<mods>(?:@[\w.]+(?:\([^\n]*\))?\s+)*(?:{modifier}\s+)*)(?P<kind>class|interface|object|typealias|fun|val|var)\s+(?:<[^<>\n]*>\s*)?(?:[A-Za-z_][\w.]*(?:<[^<>\n]*>)?\s*\.\s*)?(?P<name>`[^`\n]+`|[A-Za-z_]\w*)"
    ))
    .unwrap()
});
static TEST_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@(?:Test|ParameterizedTest|RepeatedTest|TestFactory|TestTemplate)\b").unwrap()
});

fn is_keyword(w: &str) -> bool {
    KEYWORDS.contains(&w)
}

fn ident(n: Node) -> String {
    n.text().trim_matches('`').to_string()
}

fn modifier_has(node: Node, kind: K) -> bool {
    node.child(K::MODIFIER_LIST)
        .map(|m| m.children().any(|c| c.kind() == kind))
        .unwrap_or(false)
}

fn annotations(node: Node) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(m) = node.child(K::MODIFIER_LIST) {
        for a in m.children().filter(|c| c.kind() == K::ANNOTATION_ENTRY) {
            if let Some(callee) = a.child(K::CONSTRUCTOR_CALLEE)
                && let Some(t) = callee.descendants().rfind(|d| d.kind() == K::IDENTIFIER)
            {
                out.push(ident(t));
            }
        }
    }
    out
}

/// Tokens whose text never holds code words: comments, string text, char literals.
fn is_text_token(k: K) -> bool {
    matches!(
        k,
        K::BLOCK_COMMENT
            | K::EOL_COMMENT
            | K::SHEBANG_COMMENT
            | K::DOC_COMMENT
            | K::REGULAR_STRING_PART
            | K::ESCAPE_SEQUENCE
            | K::CHARACTER_LITERAL
            | K::OPEN_QUOTE
            | K::CLOSING_QUOTE
            | K::WHITE_SPACE
    )
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    IDENT_RE.find_iter(text).map(|m| m.as_str())
}

/// Supertype names (outside `<>` and `(...)`) and the names inside their type arguments.
fn supertypes(node: Node) -> (Vec<String>, Vec<String>) {
    let Some(list) = node.child(K::SUPER_TYPE_LIST) else {
        return (vec![], vec![]);
    };
    let mut outside: Vec<String> = Vec::new();
    let mut inside: Vec<String> = Vec::new();
    let scan = |root: Node, outside: &mut Vec<String>, inside: &mut Vec<String>| {
        let mut walk = root.preorder();
        let mut depth_args: Vec<std::ops::Range<usize>> = Vec::new();
        while let Some(ev) = walk.next() {
            match ev {
                WalkEvent::Enter(n) => {
                    let k = n.kind();
                    if k == K::VALUE_ARGUMENT_LIST || k == K::LAMBDA_ARGUMENT || k == K::CLASS_BODY
                    {
                        walk.skip_subtree();
                        continue;
                    }
                    if k == K::TYPE_ARGUMENT_LIST {
                        depth_args.push(n.range());
                    }
                    if n.is_token() && !is_text_token(k) {
                        let into = if depth_args.is_empty() {
                            &mut *outside
                        } else {
                            &mut *inside
                        };
                        for w in words(n.text()) {
                            into.push(w.to_string());
                        }
                    }
                }
                WalkEvent::Leave(n) => {
                    if n.kind() == K::TYPE_ARGUMENT_LIST {
                        depth_args.pop();
                    }
                }
            }
        }
    };
    scan(list, &mut outside, &mut inside);
    if let Some(c) = node.child(K::TYPE_CONSTRAINT_LIST) {
        scan(c, &mut outside, &mut inside);
    }
    let dedup = |v: Vec<String>| -> Vec<String> {
        let mut seen = HashSet::new();
        v.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };
    (dedup(outside), dedup(inside))
}

/// `object : a.b.Foo<T>()` -> "Foo": the first supertype's simple name, as the reference regex reads it.
fn anon_supertype(obj_literal: Node) -> Option<String> {
    let decl = obj_literal.child(K::OBJECT_DECLARATION)?;
    let list = decl.child(K::SUPER_TYPE_LIST)?;
    let entry = list.child_nodes().next()?;
    let ut = entry.descendants().find(|d| d.kind() == K::USER_TYPE)?;
    user_type_path(ut).pop()
}

/// Segments of a (possibly qualified) user type, outermost qualifier first.
fn user_type_path(ut: Node) -> Vec<String> {
    let mut segs = Vec::new();
    let mut cur = Some(ut);
    while let Some(u) = cur {
        if let Some(r) = u.child(K::REFERENCE_EXPRESSION) {
            segs.push(ident(r));
        }
        cur = u.child(K::USER_TYPE);
    }
    segs.reverse();
    segs
}

/// `fun Outer.Inner<T>?.f()` -> [Outer, Inner].
fn receiver_path(node: Node) -> Vec<String> {
    let mut tr = None;
    for c in node.children() {
        if c.kind() == K::IDENTIFIER {
            break;
        }
        if c.kind() == K::TYPE_REFERENCE {
            tr = Some(c);
        }
    }
    let Some(tr) = tr else { return vec![] };
    let mut t = tr.child_nodes().find(|c| c.kind() != K::MODIFIER_LIST);
    while let Some(n) = t {
        match n.kind() {
            K::USER_TYPE => return user_type_path(n),
            K::NULLABLE_TYPE => t = n.child_nodes().find(|c| c.kind() != K::MODIFIER_LIST),
            _ => return vec![],
        }
    }
    vec![]
}

struct Frame {
    decl: usize,
    range: std::ops::Range<usize>,
    kind: u8,
}

fn decl_kind(node: Node) -> Option<u8> {
    Some(match node.kind() {
        K::CLASS => {
            if node.children().any(|c| c.kind() == K::INTERFACE_KEYWORD) {
                KIND_INTERFACE
            } else {
                KIND_CLASS
            }
        }
        K::OBJECT_DECLARATION => KIND_OBJECT,
        K::FUN => KIND_FUN,
        K::PROPERTY => {
            if node.children().any(|c| c.kind() == K::VAR_KEYWORD) {
                KIND_VAR
            } else {
                KIND_VAL
            }
        }
        K::TYPEALIAS => KIND_TYPEALIAS,
        K::ENUM_ENTRY => KIND_ENUM_ENTRY,
        K::CLASS_INITIALIZER => KIND_INIT,
        K::SECONDARY_CONSTRUCTOR => KIND_CONSTRUCTOR,
        _ => return None,
    })
}

fn is_ctor_property(node: Node) -> bool {
    node.kind() == K::VALUE_PARAMETER
        && node
            .parent()
            .and_then(|p| p.parent())
            .map(|pp| pp.kind() == K::PRIMARY_CONSTRUCTOR)
            .unwrap_or(false)
        && node
            .children()
            .any(|c| matches!(c.kind(), K::VAL_KEYWORD | K::VAR_KEYWORD))
}

fn callee_name(call: Node<'_>) -> Option<&str> {
    let first = call.child_nodes().next()?;
    (first.kind() == K::REFERENCE_EXPRESSION).then(|| first.text())
}

fn is_registration_call(n: Node) -> bool {
    n.kind() == K::CALL_EXPRESSION
        && matches!(
            callee_name(n),
            Some("polymorphic" | "polymorphicDefaultDeserializer" | "polymorphicDefaultSerializer")
        )
}

/// The receiver of `X::class` or `X.serializer()` inside a polymorphic registration.
fn is_registered_type(n: Node) -> bool {
    let Some(p) = n.parent() else { return false };
    if p.first_child().map(|c| c.range()) != Some(n.range()) {
        return false;
    }
    match p.kind() {
        K::CLASS_LITERAL_EXPRESSION => true,
        K::DOT_QUALIFIED_EXPRESSION => p
            .child_nodes()
            .last()
            .map(|s| s.kind() == K::CALL_EXPRESSION && callee_name(s) == Some("serializer"))
            .unwrap_or(false),
        _ => false,
    }
}

/// Metro `excludes = [...]` / `replaces = [...]` name bindings a graph does NOT use.
fn is_excluded_bindings(n: Node) -> bool {
    n.kind() == K::VALUE_ARGUMENT
        && n.child(K::VALUE_ARGUMENT_NAME)
            .map(|a| matches!(a.text().trim(), "excludes" | "replaces"))
            .unwrap_or(false)
        && n.child_nodes()
            .any(|c| c.kind() == K::COLLECTION_LITERAL_EXPRESSION)
}

/// Top-level names the runner can be handed as test classes (kotlin_index.parse_file's rule).
fn test_class_names(text: &str) -> HashSet<String> {
    let spans: Vec<(usize, String, bool)> = TOP_DECL_RE
        .captures_iter(text)
        .map(|c| {
            let m = c.get(0).unwrap();
            let mods = c.name("mods").map(|x| x.as_str()).unwrap_or("");
            let kind = &c["kind"];
            let is_class =
                matches!(kind, "class" | "interface" | "object") && !mods.contains("companion");
            (
                m.start(),
                c["name"].trim_matches('`').to_string(),
                is_class && !mods.contains("abstract"),
            )
        })
        .collect();
    let has_marker = TEST_MARKER_RE.is_match(text);
    let mut out = HashSet::new();
    for (i, (start, name, emittable)) in spans.iter().enumerate() {
        if !emittable {
            continue;
        }
        let end = spans.get(i + 1).map(|s| s.0).unwrap_or(text.len());
        let named = name.ends_with("Test") || name.ends_with("Tests");
        if TEST_MARKER_RE.is_match(&text[*start..end]) || (has_marker && named) {
            out.insert(name.clone());
        }
    }
    out
}

pub fn parse_file(text: &str, _test_source: bool) -> RawFile {
    let file = match kt_syntax::parse(text) {
        Ok(f) => f,
        Err(_) => {
            return RawFile {
                error: true,
                ..Default::default()
            };
        }
    };
    let root = file.root();
    let line = |off: usize| file.line_col(off).map(|l| l.line + 1).unwrap_or(0);
    let mut fp = RawFile {
        error: file.has_errors(),
        ..Default::default()
    };
    if let Some(p) = root.child(K::PACKAGE_DIRECTIVE) {
        fp.package = p
            .descendants()
            .filter(|d| d.kind() == K::IDENTIFIER)
            .map(ident)
            .collect::<Vec<_>>()
            .join(".");
    }
    for imp in root.find_all(&[K::IMPORT_DIRECTIVE]) {
        let alias_node = imp.child(K::IMPORT_ALIAS);
        let alias = alias_node
            .and_then(|a| a.descendants().find(|d| d.kind() == K::IDENTIFIER))
            .map(ident);
        let segs: Vec<String> = imp
            .descendants()
            .filter(|d| d.kind() == K::IDENTIFIER)
            .filter(|d| {
                alias_node
                    .map(|a| !a.range().contains(&d.range().start))
                    .unwrap_or(true)
            })
            .map(ident)
            .collect();
        let star = imp.descendants().any(|d| d.kind() == K::MUL);
        fp.imports.push(RawImport { segs, alias, star });
    }

    let mut frames: Vec<Frame> = Vec::new();
    let mut ref_sets: Vec<HashSet<String>> = Vec::new();
    // Name-only spans (constructor properties) whose name token no declaration owns.
    let mut name_only: Vec<std::ops::Range<usize>> = Vec::new();
    let mut own_mods: Option<std::ops::Range<usize>> = None;
    let mut registration_depth: Vec<std::ops::Range<usize>> = Vec::new();
    let mut code_words: HashSet<String> = HashSet::new();
    // Per declaration: (lead, keyword offset, end) in the source, for the text scans.
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    let mut masked: Vec<u8> = text.as_bytes().to_vec();
    let blank = |r: std::ops::Range<usize>, masked: &mut Vec<u8>| {
        for c in &mut masked[r] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let mut walk = root.preorder();
    while let Some(ev) = walk.next() {
        match ev {
            WalkEvent::Enter(node) => {
                let k = node.kind();
                if k == K::DOC_COMMENT {
                    blank(node.range(), &mut masked);
                    walk.skip_subtree();
                    continue;
                }
                if k == K::PACKAGE_DIRECTIVE || k == K::IMPORT_LIST {
                    walk.skip_subtree();
                    continue;
                }
                if node.is_token() {
                    if is_text_token(k) {
                        if k != K::WHITE_SPACE {
                            blank(node.range(), &mut masked);
                        }
                        continue;
                    }
                    let r = node.range();
                    let in_mods = own_mods
                        .as_ref()
                        .map(|m| m.start <= r.start && r.end <= m.end)
                        .unwrap_or(false);
                    let named_only = name_only.contains(&r);
                    for w in words(node.text()) {
                        code_words.insert(w.to_string());
                        if is_keyword(w) || named_only || (in_mods && MODIFIERS.contains(&w)) {
                            continue;
                        }
                        if let Some(f) = frames.last() {
                            ref_sets[f.decl].insert(w.to_string());
                        }
                    }
                    continue;
                }
                if is_excluded_bindings(node) {
                    walk.skip_subtree();
                    continue;
                }
                if is_registration_call(node) {
                    registration_depth.push(node.range());
                }
                if !registration_depth.is_empty() && is_registered_type(node) {
                    walk.skip_subtree();
                    continue;
                }
                if k == K::MODIFIER_LIST
                    && let (Some(f), Some(p)) = (frames.last(), node.parent())
                    && p.range() == f.range
                {
                    own_mods = Some(node.range());
                }
                if k == K::OBJECT_LITERAL
                    && let (Some(f), Some(name)) = (frames.last(), anon_supertype(node))
                {
                    let anon = &mut fp.decls[f.decl].anon;
                    if !anon.contains(&name) {
                        anon.push(name);
                    }
                }
                let top_container = frames
                    .last()
                    .map(|f| is_container_kind(f.kind))
                    .unwrap_or(true);
                if !top_container {
                    continue;
                }
                let structural = match node.parent() {
                    None => false,
                    Some(p) if p.kind() == K::KT_FILE => frames.is_empty(),
                    Some(p) if p.kind() == K::CLASS_BODY => frames
                        .last()
                        .map(|f| p.parent().map(|pp| pp.range() == f.range).unwrap_or(false))
                        .unwrap_or(false),
                    _ => false,
                };
                if is_ctor_property(node) {
                    let owner = node
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|pc| pc.parent());
                    let Some(f) = frames.last() else { continue };
                    if owner.map(|o| o.range()) != Some(f.range.clone()) {
                        continue;
                    }
                    let Some(name_tok) = node.child(K::IDENTIFIER) else {
                        continue;
                    };
                    name_only.push(name_tok.range());
                    let mut flags = FLAG_CTOR_PROP | FLAG_INITIALIZED;
                    if modifier_has(node, K::PRIVATE_KEYWORD) {
                        flags |= FLAG_PRIVATE;
                    }
                    if modifier_has(node, K::OVERRIDE_KEYWORD) {
                        flags |= FLAG_OVERRIDE;
                    }
                    let l = line(name_tok.range().start);
                    let kind = if node.children().any(|c| c.kind() == K::VAR_KEYWORD) {
                        KIND_VAR
                    } else {
                        KIND_VAL
                    };
                    fp.decls.push(RawDecl {
                        name: ident(name_tok),
                        kind,
                        flags,
                        parent: f.decl as i32,
                        lo: l,
                        hi: l,
                        ..Default::default()
                    });
                    ref_sets.push(HashSet::new());
                    spans.push((
                        name_tok.range().start,
                        name_tok.range().start,
                        name_tok.range().end,
                    ));
                    continue;
                }
                if !structural {
                    continue;
                }
                let Some(kind) = decl_kind(node) else {
                    continue;
                };
                let name_tok = node.child(K::IDENTIFIER);
                let name = match (name_tok, kind) {
                    (Some(t), _) => ident(t),
                    (None, KIND_OBJECT) => "Companion".to_string(),
                    (None, KIND_INIT) => "init".to_string(),
                    (None, KIND_CONSTRUCTOR) => "constructor".to_string(),
                    _ => continue,
                };
                let mut flags = 0;
                if modifier_has(node, K::PRIVATE_KEYWORD) {
                    flags |= FLAG_PRIVATE;
                }
                if modifier_has(node, K::OVERRIDE_KEYWORD) {
                    flags |= FLAG_OVERRIDE;
                }
                if is_type_kind(kind) {
                    if modifier_has(node, K::INNER_KEYWORD) {
                        flags |= FLAG_INNER;
                    }
                    if modifier_has(node, K::SEALED_KEYWORD) {
                        flags |= FLAG_SEALED;
                    }
                    if modifier_has(node, K::DATA_KEYWORD)
                        || modifier_has(node, K::VALUE_KEYWORD)
                        || annotations(node).iter().any(|a| a == "Serializable")
                    {
                        flags |= FLAG_ANY_SEMANTICS;
                    }
                }
                // Any `=` or `by` outside strings and comments, accessors included: an edited
                // `get() = ..` counts as a change to the type, so tests constructing it still run.
                if matches!(kind, KIND_VAL | KIND_VAR)
                    && node.tokens().any(|t| {
                        !t.is_trivia()
                            && !matches!(t.kind(), K::REGULAR_STRING_PART | K::ESCAPE_SEQUENCE)
                            && !t.kind().name().starts_with("KDOC")
                            && (t.kind() == K::BY_KEYWORD || t.text().contains('='))
                    })
                {
                    flags |= FLAG_INITIALIZED;
                }
                let first = node
                    .children()
                    .find(|c| !c.is_trivia())
                    .map(|c| c.range().start)
                    .unwrap_or(node.range().start);
                let mut d = RawDecl {
                    name,
                    kind,
                    flags,
                    parent: frames.last().map(|s| s.decl as i32).unwrap_or(-1),
                    lo: line(first),
                    hi: line(node.range().end.saturating_sub(1)),
                    annotations: annotations(node),
                    ..Default::default()
                };
                if is_type_kind(kind) {
                    let (s, a) = supertypes(node);
                    d.supers = s;
                    d.super_args = a;
                    // The reference reads `object : X` anywhere in a declaration's own text, so a
                    // nameless `companion object : X` also counts X as an anonymous supertype.
                    if kind == KIND_OBJECT
                        && name_tok.is_none()
                        && let Some(first) = node
                            .child(K::SUPER_TYPE_LIST)
                            .and_then(|l| l.child_nodes().next())
                        && let Some(ut) = first.descendants().find(|x| x.kind() == K::USER_TYPE)
                        && let Some(n) = user_type_path(ut).pop()
                    {
                        d.anon.push(n);
                    }
                }
                if d.parent < 0 && matches!(kind, KIND_FUN | KIND_VAL | KIND_VAR) {
                    d.receiver = receiver_path(node);
                }
                let kw = node
                    .children()
                    .find(|c| {
                        matches!(
                            c.kind(),
                            K::CLASS_KEYWORD
                                | K::INTERFACE_KEYWORD
                                | K::OBJECT_KEYWORD
                                | K::FUN_KEYWORD
                                | K::VAL_KEYWORD
                                | K::VAR_KEYWORD
                                | K::TYPE_ALIAS_KEYWORD
                                | K::INIT_KEYWORD
                                | K::CONSTRUCTOR_KEYWORD
                        )
                    })
                    .map(|c| c.range().start)
                    .unwrap_or(first);
                spans.push((first, kw, node.range().end));
                fp.decls.push(d);
                ref_sets.push(HashSet::new());
                frames.push(Frame {
                    decl: fp.decls.len() - 1,
                    range: node.range(),
                    kind,
                });
            }
            WalkEvent::Leave(node) => {
                if node.is_token() {
                    continue;
                }
                if registration_depth.last() == Some(&node.range()) && is_registration_call(node) {
                    registration_depth.pop();
                }
                if own_mods.as_ref() == Some(&node.range()) && node.kind() == K::MODIFIER_LIST {
                    own_mods = None;
                }
                if frames
                    .last()
                    .map(|f| f.range == node.range() && decl_kind(node).is_some())
                    .unwrap_or(false)
                {
                    frames.pop();
                }
            }
        }
    }
    for (d, refs) in fp.decls.iter_mut().zip(ref_sets) {
        let mut v: Vec<String> = refs.into_iter().filter(|w| *w != d.name).collect();
        v.sort();
        d.refs = v;
    }
    for c in masked.iter_mut() {
        if *c >= 0x80 {
            *c = b' ';
        }
    }
    let masked = String::from_utf8(masked).unwrap_or_default();
    let uses_koin = fp.imports.iter().any(|im| {
        im.segs.first().map(|s| s == "org").unwrap_or(false)
            && im.segs.get(1).map(|s| s == "koin").unwrap_or(false)
    });
    for (i, d) in fp.decls.iter().enumerate() {
        let (lead, start, end) = spans[i];
        if uses_koin
            && matches!(d.kind, KIND_FUN | KIND_VAL)
            && d.flags & FLAG_CTOR_PROP == 0
            && let Some((defs, mut rest)) = koin::koin_module(&masked, start, end)
        {
            for w in IDENT_RE.find_iter(&masked[lead..start.max(lead)]) {
                rest.names.insert(w.as_str().to_string());
            }
            fp.koin.push((i, defs, rest));
        }
        if d.kind == KIND_CLASS {
            fp.builds
                .push((i, koin::construction_names(&masked, start, end)));
            let params = koin::constructor_param_types(&masked, start, end);
            if !params.is_empty() {
                fp.params.push((i, params));
            }
        }
    }
    let tests = test_class_names(text);
    for d in fp.decls.iter_mut().filter(|d| d.parent < 0) {
        if tests.contains(&d.name) {
            d.flags |= FLAG_TEST_ROOT;
        }
    }
    let mut in_code: Vec<String> = fp
        .imports
        .iter()
        .filter(|im| !im.star)
        .filter_map(|im| im.alias.clone().or_else(|| im.segs.last().cloned()))
        .filter(|s| code_words.contains(s))
        .collect();
    in_code.sort();
    in_code.dedup();
    fp.in_code = in_code;
    fp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(raw: &RawFile) -> Vec<(String, i32)> {
        raw.decls
            .iter()
            .map(|d| (d.name.clone(), d.parent))
            .collect()
    }

    // ---- Kotlin readings a line-based parser gets wrong; each pins the correct one.

    fn refs_of<'a>(raw: &'a RawFile, name: &str) -> &'a Vec<String> {
        &raw.decls.iter().find(|d| d.name == name).unwrap().refs
    }

    #[test]
    fn enum_members_after_the_entries_belong_to_the_enum() {
        let src = "enum class E {\n  A,\n  B;\n  val label: String get() = name\n  companion object { fun of() = A }\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![
                ("E".into(), -1),
                ("A".into(), 0),
                ("B".into(), 0),
                ("label".into(), 0),
                ("Companion".into(), 0),
                ("of".into(), 4)
            ]
        );
    }

    #[test]
    fn locals_in_an_enum_entry_argument_lambda_are_not_declarations() {
        let src = "enum class S(val run: (Int) -> Int) {\n  A({ x -> val s = x + 1\n s }),\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![("S".into(), -1), ("run".into(), 0), ("A".into(), 0)]
        );
    }

    #[test]
    fn a_primary_constructor_keyword_on_its_own_line_keeps_its_properties() {
        let src = "class A @Inject\nconstructor(\n  val x: Int,\n) {\n  fun f() = x\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![("A".into(), -1), ("x".into(), 0), ("f".into(), 0)]
        );
    }

    #[test]
    fn an_object_expression_at_statement_start_is_not_a_companion() {
        let src = "class T {\n  private fun svc(): Api =\n    object : Api by Fake() {\n      override fun go() = 1\n    }\n  companion object { val K = 1 }\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![
                ("T".into(), -1),
                ("svc".into(), 0),
                ("Companion".into(), 0),
                ("K".into(), 2)
            ]
        );
        assert_eq!(raw.decls[1].anon, vec!["Api".to_string()]);
    }

    #[test]
    fn a_lambda_in_supertype_arguments_keeps_later_supertypes_and_drops_argument_words() {
        let src =
            "class C(clock: Clock) : Base<M>(init = M().let { it }, clock = clock), Api {\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            raw.decls[0].supers,
            vec!["Base".to_string(), "Api".to_string()]
        );
        assert_eq!(raw.decls[0].super_args, vec!["M".to_string()]);
    }

    #[test]
    fn quotes_inside_a_backticked_name_are_part_of_the_name() {
        let src = "class T {\n  fun `is described \"Credit\"`() { run() }\n  fun `the tap's frame`() = Unit\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(raw.decls[1].name, "is described \"Credit\"");
        assert_eq!(raw.decls[2].name, "the tap's frame");
        assert!(raw.decls[1].refs.contains(&"run".to_string()));
    }

    #[test]
    fn a_function_type_receiver_is_a_declaration() {
        let src = "object N {\n  private fun <T> ((T) -> String?).orGeneric(id: T): String = this(id) ?: \"\"\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(names(&raw), vec![("N".into(), -1), ("orGeneric".into(), 0)]);
    }

    #[test]
    fn a_function_named_init_is_one_function() {
        let src = "class T {\n  fun init(): Unit = setUp()\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(names(&raw), vec![("T".into(), -1), ("init".into(), 0)]);
        assert_eq!(raw.decls[1].kind, KIND_FUN);
    }

    #[test]
    fn object_expression_members_in_a_constructor_default_stay_in_the_header() {
        let src = "class E(\n  private val p: P = object : P {\n    override fun find(e: Ref) = null\n  },\n) {\n  fun canLink() = 1\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![("E".into(), -1), ("p".into(), 0), ("canLink".into(), 0)]
        );
        assert!(refs_of(&raw, "E").contains(&"find".to_string()));
    }

    #[test]
    fn generic_and_nested_annotations_belong_to_the_declaration_they_annotate() {
        let src = "interface A\n\n@DefaultBinding<Reg<*>>\nabstract class Reg<E>\n\nenum class R {\n  X,\n  @Deprecated(\"a\" + f(\"b\"), level = DeprecationLevel.WARNING)\n  Y,\n}\n";
        let raw = parse_file(src, false);
        assert!(refs_of(&raw, "Reg").contains(&"DefaultBinding".to_string()));
        assert!(refs_of(&raw, "A").is_empty());
        assert!(refs_of(&raw, "Y").contains(&"level".to_string()));
        assert!(!refs_of(&raw, "X").contains(&"level".to_string()));
    }

    #[test]
    fn test_class_root_follows_the_marker_rule() {
        let src = "package a\nimport kotlin.test.Test\nclass FooTest {\n  @Test fun t() { Foo().bar() }\n}\n";
        let raw = parse_file(src, true);
        assert_eq!(raw.decls[0].name, "FooTest");
        assert_ne!(raw.decls[0].flags & FLAG_TEST_ROOT, 0);
        assert!(raw.decls[1].refs.contains(&"Foo".to_string()));
    }

    #[test]
    fn locals_and_object_literal_members_fold_into_their_owner() {
        let src = "class A {\n  fun f() {\n    class Local(val x: Int)\n    val o = object : Clock { override fun now() = 1 }\n  }\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(names(&raw), vec![("A".into(), -1), ("f".into(), 0)]);
        assert!(raw.decls[1].refs.contains(&"Clock".to_string()));
        assert!(raw.decls[1].refs.contains(&"now".to_string()));
        assert_eq!(raw.decls[1].anon, vec!["Clock".to_string()]);
    }

    #[test]
    fn init_constructor_and_ctor_properties_are_nodes() {
        let src = "class A(private val repo: Repo, b: Int) {\n  init { repo.load() }\n  constructor() : this(Repo(), 1)\n}\n";
        let raw = parse_file(src, false);
        assert_eq!(
            names(&raw),
            vec![
                ("A".into(), -1),
                ("repo".into(), 0),
                ("init".into(), 0),
                ("constructor".into(), 0)
            ]
        );
        assert_ne!(raw.decls[1].flags & FLAG_PRIVATE, 0);
        // The property's type stays in the class header; its name does not.
        assert!(raw.decls[0].refs.contains(&"Repo".to_string()));
        assert!(!raw.decls[0].refs.contains(&"repo".to_string()));
        assert!(raw.decls[1].refs.is_empty());
    }

    #[test]
    fn soft_keywords_and_literal_fragments_are_tokens_hard_keywords_and_strings_are_not() {
        let src = "val x: Int get() = if (true) 1L else value.field // comment Foo\nval s = \"Bar $baz\"\n";
        let raw = parse_file(src, false);
        let r = &raw.decls[0].refs;
        for w in ["Int", "get", "L", "value", "field"] {
            assert!(r.contains(&w.to_string()), "{w} in {r:?}");
        }
        for w in ["if", "true", "else", "Foo", "x"] {
            assert!(!r.contains(&w.to_string()), "{w} in {r:?}");
        }
        assert_eq!(raw.decls[1].refs, vec!["baz".to_string()]);
    }

    #[test]
    fn supertypes_split_type_arguments_and_skip_constructor_arguments() {
        let src = "class Ext(g: G) : Base<Graph>(g.x), Foo by impl\n";
        let raw = parse_file(src, false);
        assert_eq!(raw.decls[0].supers, vec!["Base", "Foo", "by", "impl"]);
        assert_eq!(raw.decls[0].super_args, vec!["Graph"]);
    }

    #[test]
    fn excluded_bindings_and_registrations_are_not_tokens() {
        let src = "@DependencyGraph(excludes = [RealClock::class])\ninterface G\nval m = SerializersModule { polymorphic(Base::class) { subclass(Sub::class, CustomSer) } }\n";
        let raw = parse_file(src, false);
        assert!(!raw.decls[0].refs.contains(&"RealClock".to_string()));
        let r = &raw.decls[1].refs;
        assert!(!r.contains(&"Base".to_string()) && !r.contains(&"Sub".to_string()));
        assert!(r.contains(&"CustomSer".to_string()) && r.contains(&"subclass".to_string()));
    }

    #[test]
    fn extension_receiver_path() {
        let raw = parse_file("fun <T> Outer.Inner<T>?.f() = 1\n", false);
        assert_eq!(
            raw.decls[0].receiver,
            vec!["Outer".to_string(), "Inner".to_string()]
        );
    }
}

//! Functional or cosmetic: a touched declaration seeds nothing when its base and head syntax agree
//! once formatting, comments, KDoc, cosmetic annotations, trailing commas, redundant parentheses,
//! `this.` and consistent renames of locals are normalised away. Anything else, or any doubt, is
//! functional.

use crate::seeds::View;
use kt_syntax::{Node, SourceFile, SyntaxKind as K, WalkEvent};
use rustc_hash::{FxHashMap, FxHashSet};
use std::ops::Range;

/// One touched declaration's verdict.
#[derive(Clone, Debug)]
pub struct Verdict {
    pub node: u32,
    pub functional: bool,
    pub reason: String,
}

/// A structural declaration (the graph's unit) in one version of a file.
struct Decl<'a> {
    node: Node<'a>,
    name: String,
    lo: u32,
    /// (kind, name) path from the file plus the ordinal among same-path siblings.
    key: (String, usize),
}

fn decl_name(node: Node) -> Option<String> {
    let kind = node.kind();
    let structural_kind = matches!(
        kind,
        K::CLASS
            | K::OBJECT_DECLARATION
            | K::FUN
            | K::PROPERTY
            | K::TYPEALIAS
            | K::ENUM_ENTRY
            | K::CLASS_INITIALIZER
            | K::SECONDARY_CONSTRUCTOR
    );
    if !structural_kind {
        return None;
    }
    match (node.child(K::IDENTIFIER), kind) {
        (Some(t), _) => Some(t.text().trim_matches('`').to_string()),
        (None, K::OBJECT_DECLARATION) => Some("Companion".into()),
        (None, K::CLASS_INITIALIZER) => Some("init".into()),
        (None, K::SECONDARY_CONSTRUCTOR) => Some("constructor".into()),
        _ => None,
    }
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

fn line(file: &SourceFile, off: usize) -> u32 {
    file.line_col(off).map(|l| l.line + 1).unwrap_or(0)
}

/// The structural declarations of a file, as `parse.rs` finds them.
fn decls(file: &SourceFile) -> Vec<Decl<'_>> {
    let mut out: Vec<Decl> = Vec::new();
    let mut frames: Vec<(Range<usize>, String)> = Vec::new();
    let mut seen: FxHashMap<String, usize> = FxHashMap::default();
    let walk = file.root().preorder();
    for ev in walk {
        match ev {
            WalkEvent::Enter(node) => {
                if node.is_token() {
                    continue;
                }
                let path_of = |frames: &[(Range<usize>, String)], tag: String| {
                    let mut p: Vec<&str> = frames.iter().map(|f| f.1.as_str()).collect();
                    p.push(&tag);
                    p.join("/")
                };
                if is_ctor_property(node) {
                    let owner = node
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|pc| pc.parent());
                    let Some(f) = frames.last() else { continue };
                    if owner.map(|o| o.range()) != Some(f.0.clone()) {
                        continue;
                    }
                    let Some(name_tok) = node.child(K::IDENTIFIER) else {
                        continue;
                    };
                    let name = name_tok.text().trim_matches('`').to_string();
                    let path = path_of(&frames, format!("PROP:{name}"));
                    let ord = seen.entry(path.clone()).or_insert(0);
                    out.push(Decl {
                        node,
                        name,
                        lo: line(file, name_tok.range().start),
                        key: (path, *ord),
                    });
                    *ord += 1;
                    continue;
                }
                let structural = match node.parent() {
                    None => false,
                    Some(p) if p.kind() == K::KT_FILE => frames.is_empty(),
                    Some(p) if p.kind() == K::CLASS_BODY => frames
                        .last()
                        .map(|f| p.parent().map(|pp| pp.range() == f.0).unwrap_or(false))
                        .unwrap_or(false),
                    _ => false,
                };
                if !structural {
                    continue;
                }
                let Some(name) = decl_name(node) else {
                    continue;
                };
                let first = node
                    .children()
                    .find(|c| !c.is_trivia())
                    .map(|c| c.range().start)
                    .unwrap_or(node.range().start);
                let tag = format!("{}:{name}", node.kind().name());
                let path = path_of(&frames, tag.clone());
                let ord = seen.entry(path.clone()).or_insert(0);
                out.push(Decl {
                    node,
                    name,
                    lo: line(file, first),
                    key: (path, *ord),
                });
                *ord += 1;
                frames.push((node.range(), format!("{tag}#{}", *ord - 1)));
            }
            WalkEvent::Leave(node) => {
                if frames
                    .last()
                    .map(|f| f.0 == node.range() && decl_name(node).is_some())
                    .unwrap_or(false)
                {
                    frames.pop();
                }
            }
        }
    }
    out
}

const ANNOTATIONS: u8 = 1;
const COMMAS: u8 = 2;
const PARENS: u8 = 4;
const THIS: u8 = 8;
const ALPHA: u8 = 16;
const ALL: u8 = 31;
const STEPS: [(u8, &str); 5] = [
    (ANNOTATIONS, "cosmetic annotation"),
    (COMMAS, "trailing comma"),
    (PARENS, "redundant parentheses"),
    (THIS, "redundant `this.`"),
    (ALPHA, "renamed locals/lambda or private parameters"),
];

struct Tok<'a> {
    norm: String,
    raw: &'a str,
    off: usize,
}

fn next_code_token(t: Node) -> Option<Node> {
    let mut n = t.next_token();
    while let Some(x) = n {
        if !x.is_trivia() && !in_doc(x) {
            return Some(x);
        }
        n = x.next_token();
    }
    None
}

fn prev_code_sibling(n: Node) -> Option<Node> {
    let mut p = n.prev_sibling();
    while let Some(x) = p {
        if !x.is_trivia() {
            return Some(x);
        }
        p = x.prev_sibling();
    }
    None
}

fn in_doc(t: Node) -> bool {
    t.ancestors().any(|a| a.kind() == K::DOC_COMMENT)
}

fn code_children(n: Node) -> impl Iterator<Item = Node> {
    n.children().filter(|c| !c.is_trivia())
}

fn annotation_name(a: Node<'_>) -> Option<&str> {
    let callee = a.child(K::CONSTRUCTOR_CALLEE)?;
    callee
        .descendants()
        .rfind(|d| d.kind() == K::IDENTIFIER)
        .map(|t| t.text().trim_matches('`'))
}

fn redundant_parens(p: Node) -> bool {
    if p.kind() != K::PARENTHESIZED {
        return false;
    }
    let inner: Vec<Node> = code_children(p)
        .filter(|c| !matches!(c.kind(), K::LPAR | K::RPAR))
        .collect();
    let [inner] = inner.as_slice() else {
        return false;
    };
    let atomic = matches!(
        inner.kind(),
        K::REFERENCE_EXPRESSION
            | K::INTEGER_CONSTANT
            | K::FLOAT_CONSTANT
            | K::BOOLEAN_CONSTANT
            | K::CHARACTER_CONSTANT
            | K::NULL
            | K::STRING_TEMPLATE
            | K::THIS_EXPRESSION
    );
    // `(f)()` invokes a value; `f()` may call a function.
    let callee = p
        .parent()
        .map(|q| q.kind() == K::CALL_EXPRESSION && code_children(q).next() == Some(p))
        .unwrap_or(false);
    atomic && !callee
}

/// The scope a binder identifier opens, or None when the name is not a renameable binder (it is
/// visible outside the declaration: a public parameter, a property, a named argument's target).
fn binder_scope<'a>(id: Node<'a>, root: Node<'a>) -> Option<Node<'a>> {
    let p = id.parent()?;
    let param_owner = |vp: Node<'a>| -> Option<Node<'a>> {
        let up = vp.parent()?;
        let owner = if up.kind() == K::VALUE_PARAMETER_LIST {
            up.parent()?
        } else {
            up
        };
        match owner.kind() {
            K::FUNCTION_LITERAL | K::FOR | K::CATCH => Some(owner),
            K::FUN if owner != root => Some(owner), // a local or anonymous function
            K::FUN => {
                let private = owner
                    .child(K::MODIFIER_LIST)
                    .map(|m| m.children().any(|c| c.kind() == K::PRIVATE_KEYWORD))
                    .unwrap_or(false);
                private.then_some(owner)
            }
            _ => None,
        }
    };
    match p.kind() {
        K::VALUE_PARAMETER => param_owner(p),
        K::PROPERTY if p != root => p.parent(),
        K::DESTRUCTURING_DECLARATION_ENTRY => {
            let dd = p.parent()?;
            let up = dd.parent()?;
            if up.kind() == K::VALUE_PARAMETER {
                param_owner(up)
            } else {
                Some(up)
            }
        }
        _ => None,
    }
}

/// Names a local declaration in `root` could give a bare identifier: `this.x` -> `x` is safe only
/// for none of them.
fn local_names(root: Node, out: &mut FxHashSet<String>) {
    for d in root.descendants() {
        if d.kind() == K::IDENTIFIER
            && let Some(p) = d.parent()
            && (matches!(
                p.kind(),
                K::VALUE_PARAMETER | K::DESTRUCTURING_DECLARATION_ENTRY
            ) || (matches!(p.kind(), K::PROPERTY | K::FUN) && p != root))
        {
            out.insert(d.text().trim_matches('`').to_string());
        }
    }
    // Primary-constructor parameters are visible in member initializers and init blocks, never in a
    // function body; a `val` one holds the same value as its property, a `var` one need not once
    // something assigns it.
    if root.kind() == K::FUN {
        return;
    }
    for a in root.ancestors() {
        if let Some(pc) = a.child(K::PRIMARY_CONSTRUCTOR) {
            for vp in pc
                .descendants()
                .filter(|d| d.kind() == K::VALUE_PARAMETER && d.child(K::VAL_KEYWORD).is_none())
            {
                if let Some(t) = vp.child(K::IDENTIFIER) {
                    out.insert(t.text().trim_matches('`').to_string());
                }
            }
        }
    }
}

/// `this.x` whose `this.` can go: an unlabelled `this` selecting a plain property.
fn droppable_this(dq: Node, locals: &FxHashSet<String>) -> bool {
    if dq.kind() != K::DOT_QUALIFIED_EXPRESSION {
        return false;
    }
    let parts: Vec<Node> = code_children(dq).collect();
    let [recv, dot, sel] = parts.as_slice() else {
        return false;
    };
    recv.kind() == K::THIS_EXPRESSION
        && recv.child(K::LABEL_QUALIFIER).is_none()
        && dot.kind() == K::DOT
        && sel.kind() == K::REFERENCE_EXPRESSION
        && sel
            .child(K::IDENTIFIER)
            .map(|t| !locals.contains(t.text().trim_matches('`')))
            .unwrap_or(false)
}

/// A reference that resolves through scopes (not a member selector, named argument or type).
fn is_scoped_reference(id: Node) -> bool {
    let Some(r) = id.parent() else { return false };
    if r.kind() != K::REFERENCE_EXPRESSION {
        return false;
    }
    if r.parent()
        .map(|q| matches!(q.kind(), K::VALUE_ARGUMENT_NAME | K::USER_TYPE))
        .unwrap_or(true)
    {
        return false;
    }
    let mut e = r;
    while let Some(q) = e.parent() {
        if q.kind() == K::CALL_EXPRESSION && code_children(q).next() == Some(e) {
            e = q;
        } else {
            break;
        }
    }
    !prev_code_sibling(e)
        .map(|s| matches!(s.kind(), K::DOT | K::SAFE_ACCESS | K::COLONCOLON))
        .unwrap_or(false)
}

/// The declaration's own tokens, normalised by `opts`; nested declarations are one placeholder each.
fn stream<'a>(
    root: Node<'a>,
    nested: &FxHashSet<Range<usize>>,
    opts: u8,
    cosmetic: &[String],
    locals: &FxHashSet<String>,
) -> Vec<Tok<'a>> {
    let mut out = Vec::new();
    let mut skip_tok: FxHashSet<usize> = FxHashSet::default();
    let mut scopes: Vec<(Range<usize>, K, FxHashMap<String, usize>)> = Vec::new();
    let mut binders = 0usize;
    let mut walk = root.preorder();
    while let Some(ev) = walk.next() {
        let node = match ev {
            WalkEvent::Enter(n) => n,
            WalkEvent::Leave(n) => {
                if scopes
                    .last()
                    .map(|s| s.0 == n.range() && s.1 == n.kind())
                    .unwrap_or(false)
                {
                    scopes.pop();
                }
                continue;
            }
        };
        if node != root && nested.contains(&node.range()) && !node.is_token() {
            out.push(Tok {
                norm: "\u{27e8}decl\u{27e9}".into(),
                raw: "",
                off: node.range().start,
            });
            walk.skip_subtree();
            continue;
        }
        if node.is_trivia() {
            walk.skip_subtree();
            continue;
        }
        if !node.is_token() {
            if opts & ANNOTATIONS != 0
                && node.kind() == K::ANNOTATION_ENTRY
                && annotation_name(node)
                    .map(|a| cosmetic.iter().any(|c| c == a))
                    .unwrap_or(false)
            {
                walk.skip_subtree();
            } else if opts & PARENS != 0 && redundant_parens(node) {
                for c in node
                    .children()
                    .filter(|c| matches!(c.kind(), K::LPAR | K::RPAR))
                {
                    skip_tok.insert(c.range().start);
                }
            } else if opts & THIS != 0 && droppable_this(node, locals) {
                for c in code_children(node).take(2) {
                    for t in c.tokens() {
                        skip_tok.insert(t.range().start);
                    }
                }
            }
            continue;
        }
        let r = node.range();
        if r.start == r.end || skip_tok.contains(&r.start) {
            continue;
        }
        let k = node.kind();
        if opts & COMMAS != 0
            && k == K::COMMA
            && let Some(nx) = next_code_token(node)
            && matches!(nx.text(), ")" | "]" | "}" | "->" | ">")
        {
            continue;
        }
        let raw = node.text();
        if opts & ALPHA != 0 && k == K::IDENTIFIER {
            let name = raw.trim_matches('`');
            if let Some(scope) = binder_scope(node, root) {
                let at = scopes
                    .iter()
                    .position(|s| s.0 == scope.range() && s.1 == scope.kind());
                let i = match at {
                    Some(i) => i,
                    None => {
                        // Scopes nest: the new one sits inside every open scope that contains it.
                        let pos = scopes
                            .iter()
                            .rposition(|s| {
                                s.0.start <= scope.range().start && scope.range().end <= s.0.end
                            })
                            .map(|p| p + 1)
                            .unwrap_or(0);
                        scopes.insert(pos, (scope.range(), scope.kind(), FxHashMap::default()));
                        pos
                    }
                };
                scopes[i].2.insert(name.to_string(), binders);
                out.push(Tok {
                    norm: format!("${binders}"),
                    raw,
                    off: r.start,
                });
                binders += 1;
                continue;
            }
            if is_scoped_reference(node)
                && let Some(b) = scopes.iter().rev().find_map(|s| s.2.get(name))
            {
                out.push(Tok {
                    norm: format!("${b}"),
                    raw,
                    off: r.start,
                });
                continue;
            }
        }
        out.push(Tok {
            norm: raw.trim_matches('`').to_string(),
            raw,
            off: r.start,
        });
    }
    out
}

fn same(a: &[Tok], b: &[Tok]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.norm == y.norm)
}

struct Side<'f> {
    file: &'f SourceFile,
    decls: Vec<Decl<'f>>,
    nested: FxHashSet<Range<usize>>,
}

impl<'f> Side<'f> {
    fn new(file: &'f SourceFile) -> Side<'f> {
        let decls = decls(file);
        let nested = decls.iter().map(|d| d.node.range()).collect();
        Side {
            file,
            decls,
            nested,
        }
    }
}

fn compare(old: &Side, o: &Decl, new: &Side, n: &Decl, cosmetic: &[String]) -> (bool, String) {
    let mut locals = FxHashSet::default();
    local_names(o.node, &mut locals);
    local_names(n.node, &mut locals);
    let (a, b) = (
        stream(o.node, &old.nested, ALL, cosmetic, &locals),
        stream(n.node, &new.nested, ALL, cosmetic, &locals),
    );
    if !same(&a, &b) {
        let k = a
            .iter()
            .zip(&b)
            .position(|(x, y)| x.norm != y.norm)
            .unwrap_or(a.len().min(b.len()));
        let show = |t: Option<&Tok>| {
            t.map(|t| {
                if t.raw.is_empty() {
                    t.norm.clone()
                } else {
                    t.raw.to_string()
                }
            })
            .unwrap_or_else(|| "<end>".into())
        };
        let at = b
            .get(k)
            .map(|t| t.off)
            .or(b.last().map(|t| t.off))
            .unwrap_or(n.node.range().start);
        return (
            true,
            format!(
                "code differs at line {}: `{}` -> `{}`",
                line(new.file, at),
                show(a.get(k)),
                show(b.get(k))
            ),
        );
    }
    let mut did: Vec<&str> = Vec::new();
    let mut prev = 0u8;
    let (mut pa, mut pb) = (
        stream(o.node, &old.nested, 0, cosmetic, &locals),
        stream(n.node, &new.nested, 0, cosmetic, &locals),
    );
    if same(&pa, &pb) {
        return (false, "formatting, comments or KDoc only".into());
    }
    for (bit, label) in STEPS {
        let opts = prev | bit;
        let (sa, sb) = (
            stream(o.node, &old.nested, opts, cosmetic, &locals),
            stream(n.node, &new.nested, opts, cosmetic, &locals),
        );
        if !same(&sa, &pa) || !same(&sb, &pb) {
            did.push(label);
        }
        let done = same(&sa, &sb);
        (pa, pb, prev) = (sa, sb, opts);
        if done {
            break;
        }
    }
    (false, format!("formatting plus {}", did.join(", ")))
}

/// Verdicts for `touched` (one file's touched graph nodes), and the nodes that still seed: the
/// functional ones plus the containers a functional rider changes.
pub fn classify(
    v: &View,
    touched: &FxHashSet<u32>,
    new_text: Option<&str>,
    old_text: Option<&str>,
    cosmetic: &[String],
) -> (FxHashSet<u32>, Vec<Verdict>) {
    let all_functional = |why: &str| -> (FxHashSet<u32>, Vec<Verdict>) {
        let mut vs: Vec<Verdict> = touched
            .iter()
            .map(|&n| Verdict {
                node: n,
                functional: true,
                reason: why.into(),
            })
            .collect();
        vs.sort_by_key(|x| x.node);
        (touched.clone(), vs)
    };
    let (Some(nt), Some(ot)) = (new_text, old_text) else {
        return all_functional("new file");
    };
    let (Ok(nf), Ok(of)) = (kt_syntax::parse(nt), kt_syntax::parse(ot)) else {
        return all_functional("does not parse");
    };
    if nf.has_errors() || of.has_errors() {
        return all_functional("syntax errors");
    }
    let (new, old) = (Side::new(&nf), Side::new(&of));
    // A removed declaration changes behaviour (an override falls back to its super, an init block
    // stops running) and seeds through the neighbours its hunk touches: keep them all.
    let new_keys: FxHashSet<&(String, usize)> = new.decls.iter().map(|d| &d.key).collect();
    if let Some(gone) = old.decls.iter().find(|d| !new_keys.contains(&d.key)) {
        return all_functional(&format!(
            "a declaration was removed or renamed in this file (`{}`)",
            gone.name
        ));
    }
    let old_by_key: FxHashMap<&(String, usize), &Decl> =
        old.decls.iter().map(|d| (&d.key, d)).collect();
    let mut nodes: Vec<u32> = touched.iter().copied().collect();
    nodes.sort_unstable();
    let mut verdicts = Vec::new();
    for n in nodes {
        let i = n as usize;
        let name = v.p.str(v.name[i]);
        let found = new.decls.iter().find(|d| d.lo == v.lo[i] && d.name == name);
        let (functional, reason) = match found {
            None => (true, "not matched to a declaration".to_string()),
            Some(d) => match old_by_key.get(&d.key) {
                None => (true, "new declaration".to_string()),
                Some(o) => compare(&old, o, &new, d, cosmetic),
            },
        };
        verdicts.push(Verdict {
            node: n,
            functional,
            reason,
        });
    }
    let mut keep: FxHashSet<u32> = verdicts
        .iter()
        .filter(|x| x.functional)
        .map(|x| x.node)
        .collect();
    for n in keep.clone() {
        let mut n = n;
        while v.rides[n as usize] && v.parent[n as usize] >= 0 {
            n = v.parent[n as usize] as u32;
            keep.insert(n);
        }
    }
    for x in verdicts.iter_mut() {
        if !x.functional && keep.contains(&x.node) {
            x.functional = true;
            x.reason = format!(
                "{}; a functional member that runs at construction changed",
                x.reason
            );
        }
    }
    (keep, verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(old: &str, new: &str) -> (bool, String) {
        let (of, nf) = (
            kt_syntax::parse(old).unwrap(),
            kt_syntax::parse(new).unwrap(),
        );
        assert!(!of.has_errors() && !nf.has_errors());
        let (o, n) = (Side::new(&of), Side::new(&nf));
        let d = n.decls.iter().find(|d| d.name == "f").expect("f in new");
        let od = o.decls.iter().find(|x| x.key == d.key).expect("f in old");
        compare(&o, od, &n, d, &["Suppress".to_string()])
    }

    fn cosmetic(old: &str, new: &str) {
        let (f, why) = verdict(old, new);
        assert!(!f, "expected cosmetic, got functional: {why}");
    }

    fn functional(old: &str, new: &str) {
        let (f, why) = verdict(old, new);
        assert!(f, "expected functional, got cosmetic: {why}");
    }

    const BASE: &str = "class A(val p: Int) {\n    val q = 1\n    fun f(a: Int, b: Int): Int {\n        val s = a + b\n        return listOf(s).map { x -> x + q }.sum()\n    }\n}\n";

    #[test]
    fn formatting_comments_and_kdoc_are_cosmetic() {
        cosmetic(
            BASE,
            &BASE.replace("val s = a + b", "val s =\n            a +   b // sum"),
        );
        cosmetic(
            BASE,
            &BASE.replace("    fun f(", "    /** Adds. */\n    fun f("),
        );
    }

    #[test]
    fn local_and_lambda_renames_are_cosmetic() {
        cosmetic(
            BASE,
            &BASE
                .replace("val s = a + b", "val total = a + b")
                .replace("listOf(s)", "listOf(total)"),
        );
        cosmetic(BASE, &BASE.replace("{ x -> x + q }", "{ y -> y + q }"));
    }

    #[test]
    fn a_rename_that_captures_another_name_is_functional() {
        // `q` was the property; renaming the lambda parameter to `q` shadows it.
        functional(BASE, &BASE.replace("{ x -> x + q }", "{ q -> q + q }"));
    }

    #[test]
    fn public_parameter_rename_is_functional_private_is_cosmetic() {
        functional(
            BASE,
            &BASE.replace(
                "fun f(a: Int, b: Int): Int {\n        val s = a + b",
                "fun f(c: Int, b: Int): Int {\n        val s = c + b",
            ),
        );
        let private = BASE.replace("    fun f(", "    private fun f(");
        cosmetic(
            &private,
            &private.replace(
                "fun f(a: Int, b: Int): Int {\n        val s = a + b",
                "fun f(c: Int, b: Int): Int {\n        val s = c + b",
            ),
        );
    }

    #[test]
    fn trailing_comma_parens_this_and_suppress_are_cosmetic() {
        cosmetic(
            BASE,
            &BASE.replace(
                "fun f(a: Int, b: Int)",
                "fun f(\n        a: Int,\n        b: Int,\n    )",
            ),
        );
        cosmetic(BASE, &BASE.replace("val s = a + b", "val s = (a) + b"));
        cosmetic(BASE, &BASE.replace("x + q", "x + this.q"));
        cosmetic(
            BASE,
            &BASE.replace("    fun f(", "    @Suppress(\"MagicNumber\")\n    fun f("),
        );
    }

    #[test]
    fn this_is_kept_where_a_constructor_parameter_would_capture_the_name() {
        // In an initializer `q` is the parameter, `this.q` the doubled property.
        let init = "class A(q: Int) {\n    val q = q * 2\n    val f = this.q\n}\n";
        functional(init, &init.replace("this.q", "q"));
        let var = "class A(var q: Int) {\n    val f = this.q\n}\n";
        functional(var, &var.replace("this.q", "q"));
        // A function body never sees constructor parameters; a `val` one is its property anyway.
        let body = "class A(q: Int) {\n    val q = q * 2\n    fun f(): Int = this.q\n}\n";
        cosmetic(body, &body.replace("this.q", "q"));
        let val = "class A(val q: Int) {\n    val f = this.q\n}\n";
        cosmetic(val, &val.replace("this.q", "q"));
    }

    #[test]
    fn literals_calls_and_other_annotations_are_functional() {
        functional(BASE, &BASE.replace("listOf(s)", "listOf(s, 1)"));
        functional(
            BASE,
            &BASE
                .replace("val s = a + b", "val s = (a + b)")
                .replace("return listOf(s)", "return listOf(s * 2)"),
        );
        let log = "class A {\n    fun f() {\n        println(\"saved\")\n    }\n}\n";
        functional(log, &log.replace("saved", "stored"));
        functional(
            BASE,
            &BASE.replace("    fun f(", "    @JvmStatic\n    fun f("),
        );
    }

    #[test]
    fn reasons_name_the_normalisation_that_made_it_equal() {
        assert_eq!(
            "formatting, comments or KDoc only",
            verdict(BASE, &BASE.replace("a + b", "a +\n b")).1
        );
        let why = verdict(BASE, &BASE.replace("{ x -> x + q }", "{ y -> y + q }")).1;
        assert!(why.contains("renamed locals"), "{why}");
        let why = verdict(BASE, &BASE.replace("listOf(s)", "listOf(s * 3)")).1;
        assert!(
            why.contains("`)` -> `*`") || why.contains("-> `*`"),
            "{why}"
        );
    }

    #[test]
    fn loop_catch_and_destructuring_binders_rename() {
        let base = "class A {
    fun f(m: Map<String, Int>): Int {
        var t = 0
        for ((k, v) in m) { t += k.length + v }
        for (i in 0..3) { t += i }
        try { t += 1 } catch (e: Exception) { println(e) }
        return t
    }
}
";
        let renamed = base
            .replace(
                "((k, v) in m) { t += k.length + v }",
                "((key, value) in m) { t += key.length + value }",
            )
            .replace("(i in 0..3) { t += i }", "(j in 0..3) { t += j }")
            .replace(
                "(e: Exception) { println(e) }",
                "(ex: Exception) { println(ex) }",
            )
            .replace("var t", "var acc")
            .replace("t +=", "acc +=")
            .replace("return t", "return acc");
        cosmetic(base, &renamed);
        // `k.length` -> `key.size`: the selector is a member name, never renamed.
        functional(base, &base.replace("k.length", "k.hashCode()"));
    }

    #[test]
    fn a_nested_declaration_is_a_placeholder_in_its_container() {
        let base = "class A {\n    fun f() = 1\n}\n";
        let (of, nf) = (
            kt_syntax::parse(base).unwrap(),
            kt_syntax::parse(&base.replace("= 1", "= 2")).unwrap(),
        );
        let (o, n) = (Side::new(&of), Side::new(&nf));
        let (oa, na) = (&o.decls[0], &n.decls[0]);
        assert_eq!("A", na.name);
        assert!(!compare(&o, oa, &n, na, &[]).0);
    }
}

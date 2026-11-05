//! Koin modules and class construction, read from masked source text.
//!
//! Koin definitions are calls (`single { X(get()) }`, `singleOf(::X) { bind<Y>() }`), not
//! declarations, so they are recovered from the code text with comments and string text blanked.

use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::parse::{KEYWORDS, MODIFIERS};

/// One `single`/`factory`/`scoped`/`viewModel`/`worker` (or `…Of`) call, or (keyless) the text of
/// a module outside every definition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KoinDef {
    /// Types it is registered under, primary first; empty when the text does not say.
    pub keys: Vec<String>,
    /// Every identifier it writes outside those keys.
    pub names: BTreeSet<String>,
    pub eager: bool,
    /// Names written after a `.` whose receiver is an expression.
    pub dotted: BTreeSet<String>,
    /// `Type.name` pairs it writes.
    pub qualified: BTreeSet<String>,
    /// It opens a lambda of its own, whose implicit receiver a bare name may bind to.
    pub receivers: bool,
    /// It resolves a dependency without naming its type (`get()`, `singleOf(::X)`).
    pub untyped: bool,
}

static IDENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap());
static KOIN_HEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^{]*?=\s*module\s*([({])").unwrap());
static KOIN_DEF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(single|factory|scoped|viewModel|worker)(Of)?\b").unwrap());
static KOIN_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(binds|bind|withOptions|onClose)\b").unwrap());
static KOIN_CTOR_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\(\s*::\s*([A-Za-z_][A-Za-z0-9_]*)").unwrap());
static CLASS_LITERAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\s*::\s*class\b").unwrap());
static CLASS_LITERAL_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*::\s*class\b").unwrap());
static BIND_TYPE_ARG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bbind\s*<\s*([A-Za-z_][A-Za-z0-9_.]*)").unwrap());
static LAMBDA_PARAMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:\(?[A-Za-z0-9_\s,:<>?]*\)?\s*->)?\s*").unwrap());
static SCOPE_OR_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\.\s*(?:also|apply)\s*\{").unwrap());
static DOTTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.\s*([A-Za-z_][A-Za-z0-9_]*)").unwrap());
static UNTYPED_GET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(get|getOrNull|inject|injectOrNull)\s*\(").unwrap());
static ANNOTATION_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@[A-Za-z0-9_.:]+(?:\([^()]*\))?").unwrap());
static IDENT_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*").unwrap());
static IDENT_OR_TICK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`\n]+`|[A-Za-z_][A-Za-z0-9_]*").unwrap());
static SUPER_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_.]*)").unwrap());

fn b(m: &str, i: usize) -> u8 {
    m.as_bytes().get(i).copied().unwrap_or(0)
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

pub fn skip_ws(m: &str, mut i: usize, end: usize) -> usize {
    while i < end && matches!(b(m, i), b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    i
}

/// Index just past the group opening at `i`, or `i` itself if it does not close within `limit`.
pub fn skip_balanced(m: &str, i: usize, end: usize, open: u8, close: u8, limit: usize) -> usize {
    let mut depth = 0i64;
    let lim = end.min(i + limit);
    let mut j = i;
    while j < lim {
        let c = b(m, j);
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return j + 1;
            }
        }
        j += 1;
    }
    i
}

fn idents(m: &str, a: usize, z: usize) -> impl Iterator<Item = &str> {
    let z = z.min(m.len());
    let a = a.min(z);
    IDENT.find_iter(&m[a..z]).map(|x| x.as_str())
}

/// Regex search inside `m[a..z]`; offsets in `m`.
fn search(re: &Regex, m: &str, a: usize, z: usize) -> Option<(usize, usize)> {
    let z = z.min(m.len());
    if a > z {
        return None;
    }
    re.find(&m[a..z]).map(|x| (a + x.start(), a + x.end()))
}

fn top_level_parts(m: &str, start: usize, stop: usize) -> Vec<(usize, usize)> {
    let mut bounds = Vec::new();
    let mut part = start;
    let mut depth = 0i64;
    for i in start..stop {
        let c = b(m, i);
        let prev = if i > 0 { b(m, i - 1) } else { 0 };
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = (depth - 1).max(0),
            b'<' if !matches!(prev, b' ' | b'\t' | b'\r' | b'\n' | b'(' | b',' | b'=') => {
                depth += 1
            }
            b'>' if prev != b'-' => depth = (depth - 1).max(0),
            b',' if depth == 0 => {
                bounds.push((part, i));
                part = i + 1;
            }
            _ => {}
        }
    }
    bounds.push((part, stop));
    bounds
}

fn dotted(m: &str, ranges: &[(usize, usize)]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut d = BTreeSet::new();
    let mut q = BTreeSet::new();
    for &(a, z) in ranges {
        let z = z.min(m.len());
        if a >= z {
            continue;
        }
        for c in DOTTED.captures_iter(&m[a..z]) {
            let whole = c.get(0).unwrap();
            let name = c.get(1).unwrap().as_str();
            let mut j = a + whole.start();
            while j > a && matches!(b(m, j - 1), b' ' | b'\t' | b'\r' | b'\n') {
                j -= 1;
            }
            let mut k = j;
            while k > a && is_word(b(m, k - 1)) {
                k -= 1;
            }
            if k < j && b(m, k).is_ascii_uppercase() {
                q.insert(format!("{}.{name}", &m[k..j]));
            } else {
                d.insert(name.to_string());
            }
        }
    }
    (d, q)
}

fn names_of(m: &str, ranges: &[(usize, usize)]) -> BTreeSet<String> {
    ranges
        .iter()
        .flat_map(|&(a, z)| idents(m, a, z))
        .filter(|w| !KEYWORDS.contains(w))
        .map(str::to_string)
        .collect()
}

/// X when a lambda body is the single expression `X(…)` (optionally `.also { … }`).
fn constructed(m: &str, a: usize, z: usize) -> Option<String> {
    let i = search(&LAMBDA_PARAMS, m, a, z).map(|x| x.1).unwrap_or(a);
    let (s, e) = search(&IDENT_AT, m, i, z)?;
    if !b(m, s).is_ascii_uppercase() {
        return None;
    }
    let j = skip_ws(m, e, z);
    if b(m, j) != b'(' || j >= z {
        return None;
    }
    let close = skip_balanced(m, j, z, b'(', b')', z - j);
    if close == j {
        return None;
    }
    let mut k = skip_ws(m, close, z);
    while k < z {
        let (_, ce) = search(&SCOPE_OR_CHAIN, m, k, z)?;
        let brace = ce - 1;
        let close = skip_balanced(m, brace, z, b'{', b'}', z - brace);
        if close == brace {
            return None;
        }
        k = skip_ws(m, close, z);
    }
    Some(m[s..e].to_string())
}

fn definition(
    m: &str,
    d_start: usize,
    d_end: usize,
    of: bool,
    end: usize,
) -> Option<(KoinDef, usize)> {
    let mut j = skip_ws(m, d_end, end);
    let mut keys: Vec<String> = Vec::new();
    let mut key_spans: Vec<(usize, usize)> = Vec::new();
    let typed = !of && b(m, j) == b'<' && j < end;
    if typed {
        let close = skip_balanced(m, j, end, b'<', b'>', 4000);
        let k = search(&IDENT_AT, m, skip_ws(m, j + 1, close), close);
        if close == j || k.is_none() {
            return None;
        }
        let (ks, ke) = k.unwrap();
        keys.push(m[ks..ke].to_string());
        key_spans.push((j, close));
        j = skip_ws(m, close, end);
    }
    let mut args = None;
    let mut receivers = false;
    if b(m, j) == b'(' && j < end {
        let close = skip_balanced(m, j, end, b'(', b')', end - j);
        if close == j {
            return None;
        }
        args = Some((j, close));
        j = skip_ws(m, close, end);
    }
    let mut options: Vec<(usize, usize)> = Vec::new();
    let primary: Option<String>;
    if of {
        let (aa, az) = args?;
        primary = search(&KOIN_CTOR_REF, m, aa, az)
            .map(|(s, e)| {
                let c = KOIN_CTOR_REF.captures(&m[s..e]).unwrap();
                c[1].to_string()
            })
            .filter(|n| n.as_bytes()[0].is_ascii_uppercase());
        if b(m, j) == b'{' && j < end {
            let close = skip_balanced(m, j, end, b'{', b'}', end - j);
            if close == j {
                return None;
            }
            options.push((j, close));
            j = close;
        }
    } else {
        if b(m, j) != b'{' || j >= end {
            return None;
        }
        let close = skip_balanced(m, j, end, b'{', b'}', end - j);
        if close == j {
            return None;
        }
        primary = if typed {
            None
        } else {
            constructed(m, j + 1, close - 1)
        };
        receivers = m[j + 1..close - 1].contains('{');
        j = close;
    }
    let mut stop = j;
    loop {
        let p0 = skip_ws(m, stop, end);
        let Some((ts, te)) = search(&KOIN_TAIL, m, p0, end) else {
            break;
        };
        let word = &m[ts..te];
        let p = skip_ws(m, te, end);
        if word == "bind" {
            let Some((ls, le)) = search(&CLASS_LITERAL_AT, m, p, end) else {
                break;
            };
            let c = CLASS_LITERAL_AT.captures(&m[ls..le]).unwrap();
            keys.push(c[1].to_string());
            key_spans.push((ts, le));
            stop = le;
            continue;
        }
        if word == "binds" {
            let q = match search(&IDENT_AT, m, p, end) {
                Some((_, ge)) => skip_ws(m, ge, end),
                None => p,
            };
            if b(m, q) != b'(' || q >= end {
                break;
            }
            let close = skip_balanced(m, q, end, b'(', b')', end - q);
            if close == q {
                break;
            }
            for c in CLASS_LITERAL.captures_iter(&m[q..close]) {
                keys.push(c[1].to_string());
            }
            key_spans.push((ts, close));
            stop = close;
            continue;
        }
        if b(m, p) != b'{' || p >= end {
            break;
        }
        let close = skip_balanced(m, p, end, b'{', b'}', end - p);
        if close == p {
            break;
        }
        options.push((ts, close));
        stop = close;
    }
    for (a, z) in options {
        for c in BIND_TYPE_ARG.captures_iter(&m[a..z]) {
            let g = c.get(1).unwrap();
            keys.push(g.as_str().rsplit('.').next().unwrap().to_string());
            let w = c.get(0).unwrap();
            key_spans.push((a + w.start(), a + w.end()));
        }
    }
    key_spans.sort();
    let mut ranges = Vec::new();
    let mut cur = d_end;
    for (a, z) in key_spans {
        if a > cur {
            ranges.push((cur, a));
        }
        cur = cur.max(z);
    }
    ranges.push((cur, stop));
    let names = names_of(m, &ranges);
    let (dot, qual) = dotted(m, &ranges);
    let eager = m[d_start..stop.max(d_start)].contains("createdAtStart");
    let found = if typed {
        keys
    } else {
        match primary {
            None => vec![],
            Some(p) => std::iter::once(p).chain(keys).collect(),
        }
    };
    let untyped = of
        || ranges.iter().any(|&(a, z)| {
            let z = z.min(m.len());
            a < z
                && UNTYPED_GET.find_iter(&m[a..z]).any(|x| {
                    let s = a + x.start();
                    s == 0 || !(is_word(b(m, s - 1)) || b(m, s - 1) == b'.')
                })
        });
    Some((
        KoinDef {
            keys: found,
            names,
            eager,
            dotted: dot,
            qualified: qual,
            receivers,
            untyped,
        },
        stop,
    ))
}

/// The definitions of a `… = module { … }` declaration spanning `start..end`, and what it writes
/// outside every definition (as a keyless one) — None if it is not one or is `createdAtStart`.
pub fn koin_module(m: &str, start: usize, end: usize) -> Option<(Vec<KoinDef>, KoinDef)> {
    let end = end.min(m.len());
    let c = KOIN_HEAD.captures(&m[start..end])?;
    let mut i = start + c.get(1).unwrap().start();
    if b(m, i) == b'(' {
        let close = skip_balanced(m, i, end, b'(', b')', end - i);
        if close == i || m[i..close].contains("createdAtStart") {
            return None;
        }
        i = skip_ws(m, close, end);
        if b(m, i) != b'{' || i >= end {
            return None;
        }
    }
    let body_end = skip_balanced(m, i, end, b'{', b'}', end - i);
    if body_end == i {
        return None;
    }
    let mut defs = Vec::new();
    let mut rest: Vec<(usize, usize)> = vec![(start, i)];
    let mut cur = i + 1;
    let mut pos = i + 1;
    while let Some(c) = (pos < body_end)
        .then(|| KOIN_DEF.captures_at(&m[..body_end], pos))
        .flatten()
    {
        let w = c.get(0).unwrap();
        let (ds, de) = (w.start(), w.end());
        if ds > 0 && (is_word(b(m, ds - 1)) || b(m, ds - 1) == b'.') {
            pos = ds + 1;
            continue;
        }
        match definition(m, ds, de, c.get(2).is_some(), body_end) {
            None => pos = de,
            Some((d, stop)) => {
                rest.push((cur, ds));
                defs.push(d);
                cur = stop;
                pos = stop;
            }
        }
    }
    rest.push((cur, end));
    let names = names_of(m, &rest);
    let (dot, qual) = dotted(m, &rest);
    Some((
        defs,
        KoinDef {
            keys: vec![],
            names,
            eager: false,
            dotted: dot,
            qualified: qual,
            receivers: true,
            untyped: false,
        },
    ))
}

fn after_name(m: &str, start: usize, kind_len: usize, end: usize) -> usize {
    search(&IDENT_OR_TICK, m, start + kind_len, end)
        .map(|x| x.1)
        .unwrap_or(start)
}

fn skip_ctor_prefix(m: &str, mut i: usize, end: usize) -> usize {
    while i < end {
        let hit = search(&ANNOTATION_AT, m, i, end).or_else(|| search(&IDENT_AT, m, i, end));
        let Some((s, e)) = hit else { return i };
        let w = &m[s..e];
        if !w.starts_with('@') && !(MODIFIERS.contains(&w) || w == "constructor") {
            return i;
        }
        i = skip_ws(m, e, end);
    }
    i
}

fn ctor_start(m: &str, start: usize, end: usize) -> usize {
    let mut i = skip_ws(m, after_name(m, start, "class".len(), end), end);
    if b(m, i) == b'<' {
        i = skip_ws(m, skip_balanced(m, i, end, b'<', b'>', 4000), end);
    }
    skip_ctor_prefix(m, i, end)
}

fn default_eq(m: &str, a: usize, z: usize) -> Option<usize> {
    (a..z).find(|&i| {
        b(m, i) == b'='
            && !(i > 0 && matches!(b(m, i - 1), b'=' | b'!' | b'<' | b'>'))
            && !matches!(b(m, i + 1), b'=' | b'>')
    })
}

/// Names written in a class's primary-constructor parameter types.
pub fn constructor_param_types(m: &str, start: usize, end: usize) -> Vec<String> {
    let i = ctor_start(m, start, end);
    if b(m, i) != b'(' || i >= end {
        return vec![];
    }
    let close = skip_balanced(m, i, end, b'(', b')', end - i);
    if close == i {
        return vec![];
    }
    let mut out = BTreeSet::new();
    for (a, z) in top_level_parts(m, i + 1, close - 1) {
        let stop = default_eq(m, a, z).unwrap_or(z);
        if let Some(colon) = m[a..stop].rfind(':') {
            out.extend(idents(m, a + colon + 1, stop).map(str::to_string));
        }
    }
    out.into_iter()
        .filter(|w| !KEYWORDS.contains(&w.as_str()))
        .collect()
}

/// Names a class's construction runs in its header; None when the header does not parse.
pub fn construction_names(m: &str, start: usize, end: usize) -> Option<Vec<String>> {
    let mut i = ctor_start(m, start, end);
    let mut out = BTreeSet::new();
    if b(m, i) == b'(' && i < end {
        let close = skip_balanced(m, i, end, b'(', b')', end - i);
        if close == i {
            return None;
        }
        for (a, z) in top_level_parts(m, i + 1, close - 1) {
            if let Some(eq) = default_eq(m, a, z) {
                out.extend(idents(m, eq + 1, z).map(str::to_string));
            }
        }
        i = skip_ws(m, close, end);
    }
    if b(m, i) == b':' && i < end {
        let stop = m[i..end].find('{').map(|x| i + x).unwrap_or(end);
        for (a, z) in top_level_parts(m, i + 1, stop) {
            let Some((ss, se)) = search(&SUPER_ENTRY, m, a, z) else {
                continue;
            };
            let c = SUPER_ENTRY.captures(&m[ss..se]).unwrap();
            let name = c[1].to_string();
            let mut j = skip_ws(m, se, z);
            if b(m, j) == b'<' && j < z {
                j = skip_ws(m, skip_balanced(m, j, z, b'<', b'>', 4000), z);
            }
            if b(m, j) == b'(' && j < z {
                out.insert(name.rsplit('.').next().unwrap().to_string());
                out.extend(idents(m, j, z).map(str::to_string));
            } else if m[j..z.max(j)].starts_with("by") {
                out.extend(idents(m, j + 2, z).map(str::to_string));
            }
        }
    }
    Some(
        out.into_iter()
            .filter(|w| !KEYWORDS.contains(&w.as_str()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_definitions_keys_and_rest() {
        let m = "val appModule = module {\n  single<Repo> { RepoImpl(get()) }\n  factory { Service(get()) }\n  singleOf(::Cache) { bind<Store>() }\n  includes(other)\n}\n";
        let (defs, rest) = koin_module(m, 0, m.len()).unwrap();
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].keys, vec!["Repo"]);
        assert!(defs[0].names.contains("RepoImpl") && defs[0].untyped);
        assert_eq!(defs[1].keys, vec!["Service"]);
        assert_eq!(defs[2].keys, vec!["Cache", "Store"]);
        assert!(defs[2].untyped);
        assert!(rest.names.contains("includes") && rest.names.contains("other"));
        assert!(!rest.names.contains("Repo"));
    }

    #[test]
    fn eager_module_is_not_routed() {
        let m = "val m = module(createdAtStart = true) { single { A() } }";
        assert!(koin_module(m, 0, m.len()).is_none());
    }

    #[test]
    fn construction_and_param_types() {
        let m = "class A(val repo: Repo, val n: Int = defaultN()) : Base(seed()), Api by impl {";
        assert_eq!(constructor_param_types(m, 0, m.len()), vec!["Int", "Repo"]);
        assert_eq!(
            construction_names(m, 0, m.len()).unwrap(),
            vec!["Base", "defaultN", "impl", "seed"]
        );
    }
}

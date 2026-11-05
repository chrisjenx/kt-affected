//! One pull request's selection for the public accuracy replay (`replay/`), as JSON on stdout.
//!
//!   cargo run --release --example replay -- <root> --base REF --head REF [--cache-dir DIR]
//!       [--static-discovery] [--inventory-base REF]
//!
//! Prints `{"explanation": …, "inventory": …}`: `explanation` is `kt_affected::explain` for
//! base..head (the selection plus verdicts and paths); `inventory`, when `--inventory-base` names a
//! commit with an empty tree, is the selection from it, which lists every test class of every task.
//! A failed call prints `{"error": "<message>"}` in its place.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(
        args.first()
            .expect("usage: replay <root> --base REF --head REF"),
    );
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let opts = |base: String| {
        let mut o = kt_affected::SelectOptions::new()
            .base(base)
            .static_discovery(args.iter().any(|a| a == "--static-discovery"));
        if let Some(h) = flag("--head") {
            o = o.head(h);
        }
        if let Some(d) = flag("--cache-dir") {
            o = o.cache_dir(d);
        }
        o
    };
    let err = |e: kt_affected::Error| serde_json::json!({"error": e.to_string()});
    let explanation = match kt_affected::explain(&root, &opts(flag("--base").expect("--base"))) {
        Ok(e) => serde_json::to_value(e).unwrap(),
        Err(e) => err(e),
    };
    let inventory = match flag("--inventory-base") {
        Some(b) => match kt_affected::select(&root, &opts(b)) {
            Ok(s) => serde_json::to_value(s).unwrap(),
            Err(e) => err(e),
        },
        None => serde_json::Value::Null,
    };
    println!(
        "{}",
        serde_json::json!({"explanation": explanation, "inventory": inventory})
    );
}

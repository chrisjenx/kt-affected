//! One pull request's selection for the public accuracy replay (`replay/`), as JSON on stdout.
//!
//!   cargo run --release --example replay -- <root> --base REF --head REF [--cache-dir DIR]
//!       [--static-discovery]
//!
//! Prints `{"explanation": …, "inventory": …}`: `explanation` is `kt_affected::explain` for
//! base..head (the selection plus verdicts and paths); `inventory` lists every test class of every
//! task at head, as whole groups.
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
    let (explanation, inventory) =
        match kt_affected::explain(&root, &opts(flag("--base").expect("--base"))) {
            Ok(mut e) => {
                let inventory = serde_json::json!({ "groups": std::mem::take(&mut e.inventory) });
                (serde_json::to_value(e).unwrap(), inventory)
            }
            Err(e) => (err(e), serde_json::Value::Null),
        };
    println!(
        "{}",
        serde_json::json!({"explanation": explanation, "inventory": inventory})
    );
}

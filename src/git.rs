//! What changed: paths, `-U0` hunks and base-side file text, straight from git.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

/// One `git diff -U0` hunk in new-file coordinates, plus where it sat in the old file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub start: u32,
    pub count: u32,
    pub old_start: u32,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

pub struct Repo {
    /// Where git runs (`git -C`).
    pub git_dir: PathBuf,
    /// The tree selection reads: a work tree, or an extracted `head` snapshot.
    pub root: PathBuf,
    pub base: String,
    /// None: diff `base` against the working tree (plus untracked files).
    pub head: Option<String>,
    cat: Mutex<Option<(Child, ChildStdin, BufReader<ChildStdout>)>>,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

impl Repo {
    pub fn new(
        git_dir: PathBuf,
        root: PathBuf,
        base: &str,
        head: Option<String>,
    ) -> Result<Repo, String> {
        let base = git(
            &git_dir,
            &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
        )?
        .trim()
        .to_string();
        Ok(Repo {
            git_dir,
            root,
            base,
            head,
            cat: Mutex::new(None),
        })
    }

    /// The branch a change is compared with when no `--base` is given: the remote's default branch
    /// (`origin/HEAD`), else `origin/main`, else `origin/master`.
    pub fn default_upstream(git_dir: &Path) -> Result<String, String> {
        if let Ok(r) = git(
            git_dir,
            &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
        ) && let Some(b) = r.trim().strip_prefix("refs/remotes/")
        {
            return Ok(b.to_string());
        }
        ["origin/main", "origin/master"]
            .into_iter()
            .find(|b| git(git_dir, &["rev-parse", "--verify", "--quiet", b]).is_ok())
            .map(str::to_string)
            .ok_or_else(|| {
                "kt-affected: no --base given and no origin/HEAD, origin/main or origin/master to compare with".to_string()
            })
    }

    /// Merge-base of `against` and HEAD (or `head`).
    pub fn merge_base(git_dir: &Path, against: &str, head: Option<&str>) -> Result<String, String> {
        Ok(
            git(git_dir, &["merge-base", against, head.unwrap_or("HEAD")])?
                .trim()
                .to_string(),
        )
    }

    fn range(&self) -> Vec<String> {
        match &self.head {
            Some(h) => vec![self.base.clone(), h.clone()],
            None => vec![self.base.clone()],
        }
    }

    /// Every touched path, deletions included, plus untracked files in work-tree mode.
    pub fn changed(&self) -> Result<Vec<String>, String> {
        let mut args = vec![
            "diff".to_string(),
            "--name-only".into(),
            "--no-renames".into(),
        ];
        args.extend(self.range());
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut out: BTreeSet<String> = git(&self.git_dir, &a)?
            .lines()
            .map(str::to_string)
            .collect();
        if self.head.is_none() {
            out.extend(
                git(
                    &self.git_dir,
                    &["ls-files", "--others", "--exclude-standard"],
                )?
                .lines()
                .map(str::to_string),
            );
        }
        Ok(out.into_iter().filter(|l| !l.is_empty()).collect())
    }

    /// Kotlin hunks; None when git cannot answer (every changed file then seeds whole).
    pub fn hunks(&self) -> Option<std::collections::HashMap<String, Vec<Hunk>>> {
        let mut args = vec![
            "diff".to_string(),
            "--no-renames".into(),
            "--no-color".into(),
            "--no-ext-diff".into(),
            "--src-prefix=a/".into(),
            "--dst-prefix=b/".into(),
            "-U0".into(),
        ];
        args.extend(self.range());
        args.extend(["--".to_string(), "*.kt".to_string()]);
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        git(&self.git_dir, &a).ok().map(|d| parse_diff_hunks(&d))
    }

    /// The selected tree's text (work tree or snapshot).
    pub fn read(&self, path: &str) -> Option<String> {
        let b = std::fs::read(self.root.join(path)).ok()?;
        Some(String::from_utf8_lossy(&b).into_owned())
    }

    /// The base commit's text, through one long-lived `git cat-file --batch`.
    pub fn read_base(&self, path: &str) -> Option<String> {
        if path.contains('\n') {
            return None;
        }
        let mut guard = self.cat.lock().ok()?;
        if guard.is_none() {
            let mut child = Command::new("git")
                .arg("-C")
                .arg(&self.git_dir)
                .args(["cat-file", "--batch"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let stdin = child.stdin.take()?;
            let stdout = BufReader::new(child.stdout.take()?);
            *guard = Some((child, stdin, stdout));
        }
        let (_, stdin, stdout) = guard.as_mut()?;
        writeln!(stdin, "{}:{}", self.base, path).ok()?;
        stdin.flush().ok()?;
        let mut header = String::new();
        stdout.read_line(&mut header).ok()?;
        let parts: Vec<&str> = header.split_whitespace().collect();
        if parts.len() != 3 {
            return None;
        }
        let size: usize = parts[2].parse().ok()?;
        let mut body = vec![0u8; size + 1];
        stdout.read_exact(&mut body).ok()?;
        body.pop();
        (parts[1] == "blob").then(|| String::from_utf8_lossy(&body).into_owned())
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        if let Ok(mut g) = self.cat.lock()
            && let Some((mut c, _, _)) = g.take()
        {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// path -> hunks from `git diff -U0 --no-renames`. A deleted path has no entry; a path with an
/// entry but no hunks changed only its mode.
pub fn parse_diff_hunks(diff: &str) -> std::collections::HashMap<String, Vec<Hunk>> {
    let mut out: std::collections::HashMap<String, Vec<Hunk>> = Default::default();
    let mut path: Option<String> = None;
    let mut in_header = false;
    let mut cur: Option<Hunk> = None;
    let flush = |cur: &mut Option<Hunk>,
                 path: &Option<String>,
                 out: &mut std::collections::HashMap<String, Vec<Hunk>>| {
        if let (Some(h), Some(p)) = (cur.take(), path) {
            out.entry(p.clone()).or_default().push(h);
        }
    };
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            flush(&mut cur, &path, &mut out);
            path = None;
            in_header = true;
            continue;
        }
        if in_header {
            if let Some(t) = line.strip_prefix("+++ ") {
                let t = t.trim_matches('"');
                path = t.strip_prefix("b/").map(str::to_string);
                if let Some(p) = &path {
                    out.entry(p.clone()).or_default();
                }
                continue;
            }
            if !line.starts_with("@@") {
                continue;
            }
            in_header = false;
        }
        if let Some(h) = parse_header(line) {
            flush(&mut cur, &path, &mut out);
            cur = Some(h);
        } else if let Some(h) = cur.as_mut() {
            if let Some(r) = line.strip_prefix('-') {
                h.removed.push(r.to_string());
            } else if let Some(a) = line.strip_prefix('+') {
                h.added.push(a.to_string());
            }
        }
    }
    flush(&mut cur, &path, &mut out);
    out
}

fn parse_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(' ')?;
    let new = rest.strip_prefix('+')?.split(' ').next()?;
    let num = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (old_start, _) = num(old)?;
    let (start, count) = num(new)?;
    Some(Hunk {
        start,
        count,
        old_start,
        removed: vec![],
        added: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunks_in_new_coordinates() {
        let d = "diff --git a/x/A.kt b/x/A.kt\nindex 1..2 100644\n--- a/x/A.kt\n+++ b/x/A.kt\n@@ -3,2 +3 @@ fun a()\n-  old1\n-  old2\n+  new\n@@ -10,0 +11,2 @@\n+a\n+b\ndiff --git a/y/B.kt b/y/B.kt\ndeleted file mode 100644\n--- a/y/B.kt\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n";
        let h = parse_diff_hunks(d);
        let a = &h["x/A.kt"];
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].start, a[0].count, a[0].old_start), (3, 1, 3));
        assert_eq!(a[0].removed, vec!["  old1", "  old2"]);
        assert_eq!((a[1].start, a[1].count, a[1].old_start), (11, 2, 10));
        assert!(!h.contains_key("y/B.kt"));
    }
}

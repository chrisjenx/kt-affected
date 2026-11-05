//! On-disk graph and the warm / incremental / cold update.
//!
//! `graph.bin` is one memory-mapped file of little-endian u32 sections (string table, file table,
//! node table, CSR edges, name -> files index); a warm run stats the tree, maps it and stops.
//! `parses.bin` is an append-only store of each file's imports and per-declaration refs, read
//! only for the files an edit dirties.

use crate::discover::{Build, is_test_source_set};
use crate::graph::{Csr, Linker, Symbols, Visibility};
use crate::parse::{
    self, Decl, FileParse, FileRefs, Import, Interner, KoinDefI, KoinModuleI, NONE,
};
use memmap2::Mmap;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAGIC: &[u8; 8] = b"KTAFGRF1";
const FORMAT: u32 = 6;
pub const SKIP_DIRS: &[&str] = &[
    "build",
    ".git",
    ".gradle",
    ".kotlin",
    ".idea",
    ".claude",
    "node_modules",
    "generated",
];

// section ids
const S_META: usize = 0;
const S_STR_BLOB: usize = 1;
const S_STR_OFF: usize = 2;
const S_FILES: usize = 3;
const S_NODES: usize = 4;
const S_SUPERS_OFF: usize = 5;
const S_SUPERS: usize = 6;
const S_SARGS_OFF: usize = 7;
const S_SARGS: usize = 8;
const S_ANN_OFF: usize = 9;
const S_ANN: usize = 10;
/// CSR families of the linked graph, (offsets, values) section pairs in `Family` order.
const S_FAMILIES: usize = 11;
const N_FAMILIES: usize = 11;
const S_FLAGS: usize = S_FAMILIES + 2 * N_FAMILIES;
const S_DISCOVERY: usize = S_FLAGS + 1;
/// declaration -> type arguments of its supertypes, kept so an edit can rebuild the type-argument prep
const S_TARGS_OFF: usize = S_DISCOVERY + 1;
const S_TARGS: usize = S_TARGS_OFF + 1;
const N_SECTIONS: usize = S_TARGS + 1;

/// The graph's CSR families.
#[derive(Clone, Copy)]
pub enum Family {
    /// target -> referrers
    RefBy = 0,
    /// declarer -> supertypes and their inherited members
    Declares,
    /// declarer -> `object : X` supertypes
    Anon,
    /// target -> referrers its supertype walk's first hop skips
    VBlock,
    /// target -> referrers linked only through a type argument's members
    TargOnly,
    /// referrer -> type arguments its file resolved through
    TargVia,
    /// extension -> receiver types
    ExtRecv,
    /// interned member name -> declarations whose code writes it
    NameUsers,
    /// provided declaration -> [module, walk length, walk...]*
    Provides,
    /// provided class -> what constructing it runs ([NONE]: anything it names)
    Builds,
    /// provided declaration -> modules whose edge `provides` replaces
    KoinRouted,
}

const FILE_W: usize = 16;
const NODE_W: usize = 6;
// file table fields
const F_PATH: usize = 0;
// Read by selection (module grouping, test filter); written positionally below.
#[allow(dead_code)]
const F_MODULE: usize = 1;
#[allow(dead_code)]
const F_TEST: usize = 2;
const F_PKG: usize = 3;
const F_BASE: usize = 4;
const F_COUNT: usize = 5;
const F_MT: usize = 6; // 2 words
const F_SIZE: usize = 8; // 2 words
const F_HASH: usize = 10; // 2 words
const F_REC_OFF: usize = 12; // 2 words
const F_REC_LEN: usize = 14;
const F_ERROR: usize = 15;
// meta (u64): format, discovery stamp, n_files, n_nodes, n_edges, n_strings, parses live bytes
const M_FORMAT: usize = 0;
const M_STAMP: usize = 1;
const M_FILES: usize = 2;
const M_NODES: usize = 3;
const M_EDGES: usize = 4;
const M_STRINGS: usize = 5;
const M_LIVE: usize = 6;

pub struct Packed {
    mmap: Mmap,
    secs: [(usize, usize); N_SECTIONS],
}

fn le_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

impl Packed {
    pub fn open(path: &Path) -> Option<Packed> {
        let f = std::fs::File::open(path).ok()?;
        // SAFETY: the file is only replaced by rename, never written in place.
        let mmap = unsafe { Mmap::map(&f).ok()? };
        if mmap.len() < 16 + N_SECTIONS * 16 || &mmap[..8] != MAGIC {
            return None;
        }
        let mut secs = [(0usize, 0usize); N_SECTIONS];
        for (i, s) in secs.iter_mut().enumerate() {
            let at = 16 + i * 16;
            *s = (le_u64(&mmap, at) as usize, le_u64(&mmap, at + 8) as usize);
            if s.0 + s.1 > mmap.len() {
                return None;
            }
        }
        let p = Packed { mmap, secs };
        if p.meta(M_FORMAT) != FORMAT as u64 {
            return None;
        }
        Some(p)
    }

    fn bytes(&self, s: usize) -> &[u8] {
        let (o, l) = self.secs[s];
        &self.mmap[o..o + l]
    }

    fn u32s(&self, s: usize) -> &[u32] {
        bytemuck::cast_slice(self.bytes(s))
    }

    pub fn meta(&self, i: usize) -> u64 {
        let b = self.bytes(S_META);
        le_u64(b, i * 8)
    }

    pub fn n_files(&self) -> usize {
        self.meta(M_FILES) as usize
    }
    pub fn n_nodes(&self) -> usize {
        self.meta(M_NODES) as usize
    }
    pub fn n_edges(&self) -> usize {
        self.meta(M_EDGES) as usize
    }
    pub fn n_strings(&self) -> usize {
        self.meta(M_STRINGS) as usize
    }

    pub fn str(&self, i: u32) -> &str {
        let off = self.u32s(S_STR_OFF);
        let blob = self.bytes(S_STR_BLOB);
        // SAFETY: written from &str.
        unsafe {
            std::str::from_utf8_unchecked(
                &blob[off[i as usize] as usize..off[i as usize + 1] as usize],
            )
        }
    }

    fn file(&self, f: usize) -> &[u32] {
        &self.u32s(S_FILES)[f * FILE_W..(f + 1) * FILE_W]
    }

    fn file_u64(&self, f: usize, field: usize) -> u64 {
        let r = self.file(f);
        r[field] as u64 | (r[field + 1] as u64) << 32
    }

    pub fn file_path(&self, f: usize) -> &str {
        self.str(self.file(f)[F_PATH])
    }

    /// Simple names of the annotations on node `n`.
    pub fn node_annotations(&self, n: usize) -> &[u32] {
        self.csr_row(S_ANN_OFF, S_ANN, n, 1)
    }

    fn csr_row(&self, off_s: usize, val_s: usize, i: usize, width: usize) -> &[u32] {
        let off = self.u32s(off_s);
        &self.u32s(val_s)[off[i] as usize * width..off[i + 1] as usize * width]
    }

    /// Nodes that reference node `n` (CSR straight off the map).
    pub fn referenced_by(&self, n: usize) -> &[u32] {
        self.family(Family::RefBy, n)
    }

    pub fn family(&self, f: Family, row: usize) -> &[u32] {
        let off_s = S_FAMILIES + 2 * f as usize;
        let off = self.u32s(off_s);
        if row + 1 >= off.len() {
            return &[];
        }
        self.csr_row(off_s, off_s + 1, row, 1)
    }

    pub fn family_rows(&self, f: Family) -> usize {
        self.u32s(S_FAMILIES + 2 * f as usize)
            .len()
            .saturating_sub(1)
    }

    /// Linked-graph node flags (`graph::N_*`).
    pub fn node_flags(&self, n: usize) -> u8 {
        self.bytes(S_FLAGS)[n]
    }

    pub fn node(&self, n: usize) -> &[u32] {
        &self.u32s(S_NODES)[n * NODE_W..(n + 1) * NODE_W]
    }

    pub fn file_base(&self, f: usize) -> u32 {
        self.file(f)[F_BASE]
    }

    /// The file's `package` (empty for the default package). Read by selection.
    pub fn file_package(&self, f: usize) -> &str {
        self.str(self.file(f)[F_PKG])
    }

    /// Whether kt-syntax reported errors for the file. Read by selection.
    pub fn file_error(&self, f: usize) -> bool {
        self.file(f)[F_ERROR] != 0
    }

    fn decls_of(&self, f: usize) -> Vec<Decl> {
        let r = self.file(f);
        let (base, count) = (r[F_BASE] as usize, r[F_COUNT] as usize);
        (base..base + count)
            .map(|n| {
                let row = self.node(n);
                let parent = row[2] as i32;
                Decl {
                    name: row[0],
                    kind: (row[1] & 0xff) as u8,
                    flags: (row[1] >> 8 & 0xff) as u8,
                    parent: if parent < 0 { -1 } else { parent - base as i32 },
                    lo: row[4],
                    hi: row[5],
                    supers: self.csr_row(S_SUPERS_OFF, S_SUPERS, n, 1).to_vec(),
                    super_args: self.csr_row(S_SARGS_OFF, S_SARGS, n, 1).to_vec(),
                    annotations: self.csr_row(S_ANN_OFF, S_ANN, n, 1).to_vec(),
                }
            })
            .collect()
    }

    /// A CSR section pair (offsets at `off_s`, values at `off_s + 1`), copied out of the map.
    fn csr(&self, off_s: usize) -> Csr {
        Csr {
            off: self.u32s(off_s).to_vec(),
            val: self.u32s(off_s + 1).to_vec(),
        }
    }

    pub fn discovery(&self) -> &[u8] {
        self.bytes(S_DISCOVERY)
    }
}

// ------------------------------------------------------------------ refs records

fn encode_rows(rows: &[Vec<u32>], out: &mut Vec<u32>) {
    out.push(rows.len() as u32);
    for d in rows {
        out.push(d.len() as u32);
        out.extend(d);
    }
}

fn encode_refs(r: &FileRefs, out: &mut Vec<u32>) {
    out.push(r.imports.len() as u32);
    for im in &r.imports {
        out.push(im.star as u32);
        out.push(im.alias);
        out.push(im.segs.len() as u32);
        out.extend(&im.segs);
        out.extend(&im.prefixes);
    }
    encode_rows(&r.refs, out);
    encode_rows(&r.anon, out);
    encode_rows(&r.receiver, out);
    encode_rows(&r.qualified, out);
    out.push(r.in_code.len() as u32);
    out.extend(&r.in_code);
    out.push(r.koin.len() as u32);
    for m in &r.koin {
        out.push(m.decl);
        out.push(m.defs.len() as u32);
        for d in m.defs.iter().chain(std::iter::once(&m.rest)) {
            out.push(d.eager as u32 | (d.receivers as u32) << 1 | (d.untyped as u32) << 2);
            for v in [&d.keys, &d.names, &d.dotted, &d.qualified] {
                out.push(v.len() as u32);
                out.extend(v);
            }
        }
    }
    out.push(r.builds.len() as u32);
    for (i, b) in &r.builds {
        out.push(*i);
        match b {
            None => out.push(NONE),
            Some(v) => {
                out.push(v.len() as u32);
                out.extend(v);
            }
        }
    }
    out.push(r.params.len() as u32);
    for (i, v) in &r.params {
        out.push(*i);
        out.push(v.len() as u32);
        out.extend(v);
    }
}

fn decode_refs(w: &[u32]) -> FileRefs {
    let mut i = 0;
    let mut next = || {
        let v = w[i];
        i += 1;
        v
    };
    let n_imp = next() as usize;
    let mut imports = Vec::with_capacity(n_imp);
    for _ in 0..n_imp {
        let star = next() != 0;
        let alias = next();
        let n = next() as usize;
        let segs: Vec<u32> = (0..n).map(|_| next()).collect();
        let prefixes: Vec<u32> = (0..n).map(|_| next()).collect();
        imports.push(Import {
            segs,
            prefixes,
            alias,
            star,
        });
    }
    let mut rows = || -> Vec<Vec<u32>> {
        let n_decl = next() as usize;
        (0..n_decl)
            .map(|_| {
                let n = next() as usize;
                (0..n).map(|_| next()).collect()
            })
            .collect()
    };
    let refs = rows();
    let anon = rows();
    let receiver = rows();
    let qualified = rows();
    let n = next() as usize;
    let in_code = (0..n).map(|_| next()).collect();
    let vec_of = |next: &mut dyn FnMut() -> u32| -> Vec<u32> {
        let n = next() as usize;
        (0..n).map(|_| next()).collect()
    };
    let n_koin = next() as usize;
    let mut koin = Vec::with_capacity(n_koin);
    for _ in 0..n_koin {
        let decl = next();
        let n_defs = next() as usize;
        let mut defs: Vec<KoinDefI> = Vec::with_capacity(n_defs + 1);
        for _ in 0..=n_defs {
            let bits = next();
            let keys = vec_of(&mut next);
            let names = vec_of(&mut next);
            let dotted = vec_of(&mut next);
            let qualified = vec_of(&mut next);
            defs.push(KoinDefI {
                keys,
                names,
                eager: bits & 1 != 0,
                dotted,
                qualified,
                receivers: bits & 2 != 0,
                untyped: bits & 4 != 0,
            });
        }
        let rest = defs.pop().unwrap();
        koin.push(KoinModuleI { decl, defs, rest });
    }
    let n_builds = next() as usize;
    let mut builds = Vec::with_capacity(n_builds);
    for _ in 0..n_builds {
        let i = next();
        let n = next();
        builds.push((
            i,
            if n == NONE {
                None
            } else {
                Some((0..n).map(|_| next()).collect())
            },
        ));
    }
    let n_params = next() as usize;
    let mut params = Vec::with_capacity(n_params);
    for _ in 0..n_params {
        let i = next();
        params.push((i, vec_of(&mut next)));
    }
    FileRefs {
        imports,
        refs,
        anon,
        receiver,
        qualified,
        in_code,
        koin,
        builds,
        params,
    }
}

// ------------------------------------------------------------------ tree walk

pub struct Entry {
    pub path: String,
    pub mtime: u64,
    pub size: u64,
}

/// One directory listing with each entry's type, mtime and size in a single `getattrlistbulk`
/// pass, where a portable walk spends a `stat` per file.
#[cfg(target_os = "macos")]
mod bulk {
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    #[repr(C)]
    struct AttrList {
        bitmapcount: u16,
        reserved: u16,
        commonattr: u32,
        volattr: u32,
        dirattr: u32,
        fileattr: u32,
        forkattr: u32,
    }

    unsafe extern "C" {
        fn getattrlistbulk(
            dirfd: i32,
            alist: *mut AttrList,
            buf: *mut u8,
            size: usize,
            options: u64,
        ) -> i32;
    }

    const ATTR_CMN_NAME: u32 = 0x0000_0001;
    const ATTR_CMN_OBJTYPE: u32 = 0x0000_0008;
    const ATTR_CMN_MODTIME: u32 = 0x0000_0400;
    const ATTR_CMN_RETURNED_ATTRS: u32 = 0x8000_0000;
    const ATTR_FILE_DATALENGTH: u32 = 0x0000_0200;
    const FSOPT_PACK_INVAL_ATTRS: u64 = 0x0000_0008;
    const VREG: u32 = 1;
    const VDIR: u32 = 2;

    pub enum Kind {
        File,
        Dir,
        /// Symlinks and the rest: the caller stats these the portable way.
        Other,
    }

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_ne_bytes(b[at..at + 4].try_into().unwrap())
    }
    fn i64_at(b: &[u8], at: usize) -> i64 {
        i64::from_ne_bytes(b[at..at + 8].try_into().unwrap())
    }

    /// (name, kind, mtime ns, size) per entry; `None` where the volume does not support the call.
    pub fn list(dir: &Path) -> Option<Vec<(String, Kind, u64, u64)>> {
        let f = std::fs::File::open(dir).ok()?;
        let mut al = AttrList {
            bitmapcount: 5,
            reserved: 0,
            commonattr: ATTR_CMN_RETURNED_ATTRS
                | ATTR_CMN_NAME
                | ATTR_CMN_OBJTYPE
                | ATTR_CMN_MODTIME,
            volattr: 0,
            dirattr: 0,
            fileattr: ATTR_FILE_DATALENGTH,
            forkattr: 0,
        };
        let mut buf = vec![0u8; 128 * 1024];
        let mut out = Vec::new();
        loop {
            // SAFETY: `buf` outlives the call and its length is passed; `al` is a valid attrlist.
            let n = unsafe {
                getattrlistbulk(
                    f.as_raw_fd(),
                    &mut al,
                    buf.as_mut_ptr(),
                    buf.len(),
                    FSOPT_PACK_INVAL_ATTRS,
                )
            };
            if n < 0 {
                return None;
            }
            if n == 0 {
                return Some(out);
            }
            let mut at = 0usize;
            for _ in 0..n {
                let e = &buf[at..];
                let len = u32_at(e, 0) as usize;
                // length, returned attribute_set_t (5 words), then the attributes in bit order
                let name_ref = 4 + 20;
                let name_off = u32_at(e, name_ref) as i32 as isize;
                let name_len = u32_at(e, name_ref + 4) as usize;
                let start = (name_ref as isize + name_off) as usize;
                let name = String::from_utf8_lossy(&e[start..start + name_len.saturating_sub(1)])
                    .into_owned();
                let kind = match u32_at(e, name_ref + 8) {
                    VREG => Kind::File,
                    VDIR => Kind::Dir,
                    _ => Kind::Other,
                };
                let (sec, nsec) = (i64_at(e, name_ref + 12), i64_at(e, name_ref + 20));
                let size = i64_at(e, name_ref + 28);
                out.push((
                    name,
                    kind,
                    (sec as u64)
                        .wrapping_mul(1_000_000_000)
                        .wrapping_add(nsec as u64),
                    size as u64,
                ));
                at += len;
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn walk(root: &Path, rel: &str, out: &mut Vec<Entry>) {
    let dir = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let Some(listing) = bulk::list(&dir) else {
        return walk_portable(root, rel, out);
    };
    let mut subdirs = Vec::new();
    for (name, kind, mtime, size) in listing {
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        match kind {
            bulk::Kind::Dir => {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    subdirs.push(child);
                }
            }
            bulk::Kind::File if name.ends_with(".kt") => out.push(Entry {
                path: child,
                mtime,
                size,
            }),
            bulk::Kind::Other if name.ends_with(".kt") => {
                if let Ok(m) = std::fs::metadata(dir.join(&name))
                    && m.is_file()
                {
                    out.push(Entry {
                        path: child,
                        mtime: mtime_ns(&m),
                        size: m.len(),
                    });
                }
            }
            _ => {}
        }
    }
    let nested: Vec<Vec<Entry>> = subdirs
        .par_iter()
        .map(|d| {
            let mut v = Vec::new();
            walk(root, d, &mut v);
            v
        })
        .collect();
    for v in nested {
        out.extend(v);
    }
}

#[cfg(not(target_os = "macos"))]
fn walk(root: &Path, rel: &str, out: &mut Vec<Entry>) {
    walk_portable(root, rel, out)
}

fn mtime_ns(m: &std::fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn walk_portable(root: &Path, rel: &str, out: &mut Vec<Entry>) {
    let dir = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name();
        let name = name.to_string_lossy();
        let child = if rel.is_empty() {
            name.to_string()
        } else {
            format!("{rel}/{name}")
        };
        if ft.is_dir() {
            if !SKIP_DIRS.contains(&name.as_ref()) {
                subdirs.push(child);
            }
        } else if name.ends_with(".kt")
            && let Ok(m) = std::fs::metadata(dir.join(name.as_ref()))
        {
            out.push(Entry {
                path: child,
                mtime: mtime_ns(&m),
                size: m.len(),
            });
        }
    }
    let nested: Vec<Vec<Entry>> = subdirs
        .par_iter()
        .map(|d| {
            let mut v = Vec::new();
            walk(root, d, &mut v);
            v
        })
        .collect();
    for v in nested {
        out.extend(v);
    }
}

/// Stat calls contend in the kernel past a handful of threads; 8 was fastest on a 16-core Mac.
const WALK_THREADS: usize = 8;

pub fn kotlin_files(root: &Path) -> Vec<Entry> {
    let mut v = Vec::new();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(WALK_THREADS);
    match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool.install(|| walk(root, "", &mut v)),
        Err(_) => walk(root, "", &mut v),
    }
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

fn stat_key(root: &Path, rel: &str) -> (u64, u64) {
    match std::fs::metadata(root.join(rel)) {
        Ok(m) => (
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            m.len(),
        ),
        Err(_) => (0, u64::MAX),
    }
}

/// Changes to settings or any build script invalidate discovery and with it every module edge.
pub fn discovery_stamp(root: &Path, inputs: &[String]) -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(env!("CARGO_PKG_VERSION").as_bytes());
    for i in inputs {
        let (mt, sz) = stat_key(root, i);
        h.update(i.as_bytes());
        h.update(&mt.to_le_bytes());
        h.update(&sz.to_le_bytes());
    }
    h.digest()
}

// ------------------------------------------------------------------ module mapping

pub struct Modules {
    /// (dir prefix with trailing '/', module index) longest first; the root project matches only `src/`.
    prefixes: Vec<(String, u32)>,
    pub vis: Visibility,
}

impl Modules {
    pub fn new(build: &Build) -> Modules {
        let projects = &build.projects;
        let index: FxHashMap<&str, u32> = projects
            .iter()
            .enumerate()
            .map(|(i, p)| (p.path.as_str(), i as u32))
            .collect();
        let mut prefixes: Vec<(String, u32)> = build
            .source_modules()
            .iter()
            .map(|p| {
                (
                    if p.dir.is_empty() {
                        "src/".to_string()
                    } else {
                        format!("{}/", p.dir)
                    },
                    index[p.path.as_str()],
                )
            })
            .collect();
        prefixes.sort_by_key(|p| std::cmp::Reverse(p.0.len()));
        let words = projects.len().div_ceil(64).max(1);
        let visibility = build.visibility();
        let bits = projects
            .iter()
            .map(|p| {
                // A project without a build script declares nothing, so nothing narrows it.
                p.build_file.as_ref()?;
                let mut b = vec![0u64; words];
                for t in &visibility[&p.path] {
                    let i = index[t.as_str()] as usize;
                    b[i / 64] |= 1 << (i % 64);
                }
                Some(b)
            })
            .collect();
        Modules {
            prefixes,
            vis: Visibility { bits },
        }
    }

    /// (module index or NONE, in a test source set)
    pub fn locate(&self, path: &str) -> (u32, bool) {
        for (pre, m) in &self.prefixes {
            if let Some(rest) = path.strip_prefix(pre.as_str()) {
                let rest = if pre == "src/" { path } else { rest };
                let parts: Vec<&str> = rest.splitn(3, '/').collect();
                let test = parts.len() >= 3 && parts[0] == "src" && is_test_source_set(parts[1]);
                return (*m, test);
            }
        }
        (NONE, false)
    }
}

// ------------------------------------------------------------------ update

#[derive(Debug, Default, serde::Serialize)]
pub struct Report {
    pub mode: &'static str,
    pub files: usize,
    pub nodes: usize,
    pub edges: usize,
    pub reparsed: usize,
    pub relinked: usize,
    pub walk_ms: u128,
    pub parse_ms: u128,
    pub link_ms: u128,
    pub write_ms: u128,
    pub total_ms: u128,
}

enum Status {
    Clean(usize),
    Restat(usize),
    Changed(usize, String),
    New(String),
}

fn hash(b: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(b)
}

pub struct Paths {
    pub graph: PathBuf,
    pub parses: PathBuf,
}

impl Paths {
    pub fn new(dir: &Path) -> Paths {
        Paths {
            graph: dir.join("graph.bin"),
            parses: dir.join("parses.bin"),
        }
    }
}

/// Bring `dir`'s graph up to date with the tree under `root`.
pub fn update(root: &Path, build: &Build, dir: &Path, force_cold: bool) -> std::io::Result<Report> {
    let t0 = Instant::now();
    std::fs::create_dir_all(dir)?;
    let paths = Paths::new(dir);
    let stamp = discovery_stamp(root, &build.inputs);
    let old = if force_cold {
        None
    } else {
        Packed::open(&paths.graph).filter(|p| p.meta(M_STAMP) == stamp)
    };
    let entries = kotlin_files(root);
    let walk_ms = t0.elapsed().as_millis();

    // ---- classify against the old file table
    let old_idx: FxHashMap<&str, usize> = match &old {
        Some(p) => (0..p.n_files()).map(|f| (p.file_path(f), f)).collect(),
        None => FxHashMap::default(),
    };
    let statuses: Vec<Status> = entries
        .par_iter()
        .map(|e| {
            if let (Some(p), Some(&o)) = (&old, old_idx.get(e.path.as_str())) {
                if p.file_u64(o, F_MT) == e.mtime && p.file_u64(o, F_SIZE) == e.size {
                    return Status::Clean(o);
                }
                let text = std::fs::read_to_string(root.join(&e.path)).unwrap_or_default();
                if hash(text.as_bytes()) == p.file_u64(o, F_HASH) {
                    return Status::Restat(o);
                }
                return Status::Changed(o, text);
            }
            Status::New(std::fs::read_to_string(root.join(&e.path)).unwrap_or_default())
        })
        .collect();
    let deleted: Vec<usize> = match &old {
        Some(p) => {
            let live: FxHashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();
            (0..p.n_files())
                .filter(|&f| !live.contains(p.file_path(f)))
                .collect()
        }
        None => vec![],
    };
    let any_change = statuses
        .iter()
        .any(|s| matches!(s, Status::Changed(..) | Status::New(_)))
        || !deleted.is_empty();
    let any_restat = statuses.iter().any(|s| matches!(s, Status::Restat(_)));
    if let (Some(p), false) = (&old, any_change)
        && !any_restat
    {
        return Ok(Report {
            mode: "warm",
            files: p.n_files(),
            nodes: p.n_nodes(),
            edges: p.n_edges(),
            walk_ms,
            total_ms: t0.elapsed().as_millis(),
            ..Default::default()
        });
    }
    let modules = Modules::new(build);
    let t1 = Instant::now();
    let mut interner = match &old {
        Some(p) => {
            Interner::from_strings((0..p.n_strings() as u32).map(|i| p.str(i).into()).collect())
        }
        None => Interner::default(),
    };
    if let Some(p) = &old
        && let Some(r) = delta(
            &paths,
            p,
            &entries,
            &statuses,
            &deleted,
            &modules,
            &mut interner,
            t0,
            walk_ms,
        )?
    {
        return Ok(r);
    }

    // ---- parse what changed
    let texts: Vec<(usize, &str)> = statuses
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Status::Changed(_, t) | Status::New(t) => Some((i, t.as_str())),
            _ => None,
        })
        .collect();
    let mut fresh: FxHashMap<usize, FileParse> = FxHashMap::default();
    for chunk in texts.chunks(2048) {
        let raws: Vec<(usize, parse::RawFile)> = chunk
            .par_iter()
            .map(|&(i, t)| (i, parse::parse_file(t, modules.locate(&entries[i].path).1)))
            .collect();
        for (i, raw) in raws {
            fresh.insert(i, interner.intern_file(raw));
        }
    }
    let reparsed = fresh.len();
    let parse_ms = t1.elapsed().as_millis();

    // ---- symbol table and every file's tokens
    let t2 = Instant::now();
    let mut sym = Symbols::default();
    let mut file_meta: Vec<(u64, u64, u64, bool)> = Vec::with_capacity(entries.len()); // mtime, size, hash, error
    for (i, (e, st)) in entries.iter().zip(&statuses).enumerate() {
        interner.intern(&e.path);
        let (m, test) = modules.locate(&e.path);
        match st {
            Status::Clean(o) | Status::Restat(o) => {
                let p = old.as_ref().unwrap();
                push_carried(&mut sym, p, *o, m, test);
                file_meta.push((
                    e.mtime,
                    e.size,
                    p.file_u64(*o, F_HASH),
                    p.file(*o)[F_ERROR] != 0,
                ));
            }
            Status::Changed(_, t) | Status::New(t) => {
                let fp = &fresh[&i];
                sym.push_file(fp.package, m, test, &fp.decls);
                file_meta.push((e.mtime, e.size, hash(t.as_bytes()), fp.error));
            }
        }
    }
    let n_dirty = fresh.len() + deleted.len();
    let n_nodes = sym.nodes();
    let parses_old: Option<Mmap> = if old.is_some() {
        std::fs::File::open(&paths.parses)
            .ok()
            .and_then(|f| unsafe { Mmap::map(&f).ok() })
    } else {
        None
    };
    let refs_of = |i: usize| -> FileRefs {
        if let Some(fp) = fresh.get(&i) {
            return fp.refs.clone();
        }
        let o = match &statuses[i] {
            Status::Clean(o) | Status::Restat(o) => *o,
            _ => unreachable!(),
        };
        let p = old.as_ref().unwrap();
        let (off, len) = (
            p.file_u64(o, F_REC_OFF) as usize,
            p.file(o)[F_REC_LEN] as usize,
        );
        let bytes = &parses_old.as_ref().expect("parses.bin missing")[off..off + len * 4];
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        decode_refs(&words)
    };
    let prof = std::env::var_os("KT_AFFECTED_PROFILE").is_some();
    if prof {
        eprintln!("  symbols: {}ms", t2.elapsed().as_millis());
    }
    let all_refs: Vec<FileRefs> = (0..entries.len()).into_par_iter().map(refs_of).collect();
    if prof {
        eprintln!("  decode refs: {}ms", t2.elapsed().as_millis());
    }

    // ---- link
    let graph = {
        let linker = Linker::new(&sym, &modules.vis, &all_refs, &interner.strs, |s| {
            interner.get(s)
        });
        if prof {
            eprintln!("  linker new: {}ms", t2.elapsed().as_millis());
        }
        let g = linker.link(|s| interner.get(s));
        std::mem::forget(linker);
        g
    };
    std::mem::forget(all_refs);
    let n_edges = graph.refby.val.len();
    let link_ms = t2.elapsed().as_millis();

    // ---- parses.bin: append records of reparsed files; rewrite when garbage dominates
    let t3 = Instant::now();
    let mut rec: Vec<(u64, u32)> = vec![(0, 0); entries.len()];
    let old_live = old.as_ref().map(|p| p.meta(M_LIVE)).unwrap_or(0);
    let old_len = parses_old.as_ref().map(|m| m.len() as u64).unwrap_or(0);
    let rewrite = old.is_none() || parses_old.is_none() || old_len > 3 * old_live.max(1);
    let mut buf: Vec<u32> = Vec::new();
    let mut live: u64 = 0;
    let mut base_off = if rewrite { 0 } else { old_len };
    if rewrite {
        // Every file's record, carried or fresh, goes into one new file.
        let all: Vec<Vec<u32>> = (0..entries.len())
            .into_par_iter()
            .map(|i| {
                let mut w = Vec::new();
                encode_refs(&refs_of(i), &mut w);
                w
            })
            .collect();
        for (i, w) in all.into_iter().enumerate() {
            rec[i] = (base_off + buf.len() as u64 * 4, w.len() as u32);
            live += w.len() as u64 * 4;
            buf.extend(w);
        }
    } else {
        let p = old.as_ref().unwrap();
        for (i, st) in statuses.iter().enumerate() {
            match st {
                Status::Clean(o) | Status::Restat(o) => {
                    rec[i] = (p.file_u64(*o, F_REC_OFF), p.file(*o)[F_REC_LEN]);
                    live += rec[i].1 as u64 * 4;
                }
                _ => {
                    let mut w = Vec::new();
                    encode_refs(&fresh[&i].refs, &mut w);
                    rec[i] = (base_off, w.len() as u32);
                    base_off += w.len() as u64 * 4;
                    live += w.len() as u64 * 4;
                    buf.extend(w);
                }
            }
        }
    }
    drop(parses_old);
    if rewrite {
        let tmp = paths.parses.with_extension("bin.tmp");
        std::fs::write(&tmp, bytemuck::cast_slice::<u32, u8>(&buf))?;
        std::fs::rename(&tmp, &paths.parses)?;
    } else if !buf.is_empty() {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&paths.parses)?;
        f.write_all(bytemuck::cast_slice::<u32, u8>(&buf))?;
    }

    // ---- graph.bin
    let mut files_tab: Vec<u32> = Vec::with_capacity(entries.len() * FILE_W);
    for (i, e) in entries.iter().enumerate() {
        let path_id = interner.get(&e.path).expect("path interned");
        let (mt, sz, h, err) = file_meta[i];
        let range = sym.file_nodes(i);
        files_tab.extend([
            path_id,
            sym.file_module[i],
            sym.file_test[i] as u32,
            sym.file_pkg[i],
            range.start,
            range.end - range.start,
            mt as u32,
            (mt >> 32) as u32,
            sz as u32,
            (sz >> 32) as u32,
            h as u32,
            (h >> 32) as u32,
            rec[i].0 as u32,
            (rec[i].0 >> 32) as u32,
            rec[i].1,
            err as u32,
        ]);
    }
    let mut nodes_tab: Vec<u32> = Vec::with_capacity(n_nodes * NODE_W);
    for n in 0..n_nodes {
        nodes_tab.extend([
            sym.name[n],
            sym.kind[n] as u32 | (sym.flags[n] as u32) << 8,
            sym.parent[n] as u32,
            sym.file[n],
            sym.lo[n],
            sym.hi[n],
        ]);
    }
    let mut blob: Vec<u8> = Vec::new();
    let mut str_off: Vec<u32> = Vec::with_capacity(interner.strs.len() + 1);
    str_off.push(0);
    for s in &interner.strs {
        blob.extend_from_slice(s.as_bytes());
        str_off.push(blob.len() as u32);
    }
    let discovery = serde_json::to_vec(build).unwrap_or_default();
    let meta: Vec<u64> = vec![
        FORMAT as u64,
        stamp,
        entries.len() as u64,
        n_nodes as u64,
        n_edges as u64,
        interner.strs.len() as u64,
        live,
    ];
    let families: [&Csr; N_FAMILIES] = [
        &graph.refby,
        &graph.declares,
        &graph.anon,
        &graph.vblock,
        &graph.targ_only,
        &graph.targ_via,
        &graph.ext_recv,
        &graph.name_users,
        &graph.provides,
        &graph.builds,
        &graph.koin_routed,
    ];
    let mut sections: Vec<&[u8]> = vec![
        bytemuck::cast_slice(&meta),
        &blob,
        bytemuck::cast_slice(&str_off),
        bytemuck::cast_slice(&files_tab),
        bytemuck::cast_slice(&nodes_tab),
        bytemuck::cast_slice(&sym.supers.off),
        bytemuck::cast_slice(&sym.supers.val),
        bytemuck::cast_slice(&sym.super_args.off),
        bytemuck::cast_slice(&sym.super_args.val),
        bytemuck::cast_slice(&sym.annotations.off),
        bytemuck::cast_slice(&sym.annotations.val),
    ];
    for c in families {
        sections.push(bytemuck::cast_slice(&c.off));
        sections.push(bytemuck::cast_slice(&c.val));
    }
    sections.push(&graph.flags);
    sections.push(&discovery);
    sections.push(bytemuck::cast_slice(&graph.type_args.off));
    sections.push(bytemuck::cast_slice(&graph.type_args.val));
    write_packed(&paths.graph, &sections)?;
    let write_ms = t3.elapsed().as_millis();
    Ok(Report {
        mode: if old.is_some() { "incremental" } else { "cold" },
        files: entries.len(),
        nodes: n_nodes,
        edges: n_edges,
        reparsed,
        relinked: n_dirty,
        walk_ms,
        parse_ms,
        link_ms,
        write_ms,
        total_ms: t0.elapsed().as_millis(),
    })
}

/// Append old file `o`'s declarations to `sym` straight from the node table (ids rebased).
fn push_carried(sym: &mut Symbols, p: &Packed, o: usize, module: u32, test: bool) {
    let r = p.file(o);
    let (old_base, count) = (r[F_BASE] as usize, r[F_COUNT] as usize);
    let f = sym.file_base.len() as u32;
    let base = sym.name.len() as i64;
    sym.file_base.push(base as u32);
    sym.file_pkg.push(r[F_PKG]);
    sym.file_module.push(module);
    sym.file_test.push(test);
    for row in p.u32s(S_NODES)[old_base * NODE_W..(old_base + count) * NODE_W].chunks_exact(NODE_W)
    {
        sym.name.push(row[0]);
        sym.kind.push((row[1] & 0xff) as u8);
        sym.flags.push((row[1] >> 8 & 0xff) as u8);
        let parent = row[2] as i32;
        sym.parent.push(if parent < 0 {
            -1
        } else {
            (parent as i64 - old_base as i64 + base) as i32
        });
        sym.file.push(f);
        sym.lo.push(row[4]);
        sym.hi.push(row[5]);
    }
    for (c, off_s, val_s) in [
        (&mut sym.supers, S_SUPERS_OFF, S_SUPERS),
        (&mut sym.super_args, S_SARGS_OFF, S_SARGS),
        (&mut sym.annotations, S_ANN_OFF, S_ANN),
    ] {
        if c.off.is_empty() {
            c.off.push(0);
        }
        let off = p.u32s(off_s);
        let (a, b) = (off[old_base] as usize, off[old_base + count] as usize);
        let shift = c.val.len() as i64 - a as i64;
        c.val.extend_from_slice(&p.u32s(val_s)[a..b]);
        c.off.extend(
            off[old_base + 1..old_base + count + 1]
                .iter()
                .map(|&x| (x as i64 + shift) as u32),
        );
    }
}

/// Files an edit may change before the delta path stops paying for itself.
const DELTA_MAX_FILES: usize = 32;

/// Swap the changed files' own pairs into the old graph, when nothing they changed reaches past them.
///
/// A file's pairs in `RefBy`/`VBlock`/`TargOnly`/`NameUsers` are the ones whose value is one of its
/// declarations, and its `TargVia` rows are its declarations' rows (`Linker::assemble`). Every other
/// family and every node flag is a function of the declarations, supertypes, type arguments,
/// receivers, construction and Koin wiring. So when an edit keeps a file's declarations (all but
/// their lines), has no Koin module on either side and links to the same outside-reaching results,
/// the cold graph is the old graph with that file's pairs replaced. `None` hands the update to a
/// full relink.
#[allow(clippy::too_many_arguments)]
fn delta(
    paths: &Paths,
    p: &Packed,
    entries: &[Entry],
    statuses: &[Status],
    deleted: &[usize],
    modules: &Modules,
    interner: &mut Interner,
    t0: Instant,
    walk_ms: u128,
) -> std::io::Result<Option<Report>> {
    let prof = std::env::var_os("KT_AFFECTED_PROFILE").is_some();
    let bail = |why: &str| {
        if prof {
            eprintln!("  delta: full relink ({why})");
        }
        Ok(None)
    };
    if !deleted.is_empty() || entries.len() != p.n_files() {
        return bail("files added or deleted");
    }
    let mut changed: Vec<(usize, &str)> = Vec::new();
    for (i, st) in statuses.iter().enumerate() {
        match st {
            Status::New(_) => return bail("files added or deleted"),
            Status::Changed(o, t) => {
                if *o != i {
                    return bail("file order moved");
                }
                changed.push((i, t.as_str()));
            }
            Status::Clean(o) | Status::Restat(o) if *o != i => return bail("file order moved"),
            _ => {}
        }
    }
    if changed.is_empty() || changed.len() > DELTA_MAX_FILES {
        return bail("no edit, or too many files");
    }
    let parses_old = match std::fs::File::open(&paths.parses)
        .ok()
        .and_then(|f| unsafe { Mmap::map(&f).ok() })
    {
        Some(m) => m,
        None => return bail("parses.bin missing"),
    };
    let old_refs = |o: usize| -> FileRefs {
        let (off, len) = (
            p.file_u64(o, F_REC_OFF) as usize,
            p.file(o)[F_REC_LEN] as usize,
        );
        let words: Vec<u32> = parses_old[off..off + len * 4]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        decode_refs(&words)
    };

    // ---- reparse, and keep only edits that leave every declaration in place
    let t1 = Instant::now();
    let lap = |what: &str, t: Instant| {
        if prof {
            eprintln!("  delta {what}: {}ms", t.elapsed().as_millis());
        }
    };
    let raws: Vec<parse::RawFile> = changed
        .par_iter()
        .map(|&(i, t)| parse::parse_file(t, modules.locate(&entries[i].path).1))
        .collect();
    let mut fresh: FxHashMap<usize, (FileParse, FileRefs)> = FxHashMap::default();
    for (&(i, _), raw) in changed.iter().zip(raws) {
        let fp = interner.intern_file(raw);
        let old_decls = p.decls_of(i);
        let same_shape = fp.package == p.file(i)[F_PKG]
            && fp.error == (p.file(i)[F_ERROR] != 0)
            && fp.decls.len() == old_decls.len()
            && fp.decls.iter().zip(&old_decls).all(|(a, b)| {
                a.name == b.name
                    && a.kind == b.kind
                    && a.flags == b.flags
                    && a.parent == b.parent
                    && a.supers == b.supers
                    && a.super_args == b.super_args
                    && a.annotations == b.annotations
            });
        if !same_shape {
            let first = fp.decls.iter().zip(&old_decls).position(|(a, b)| {
                (
                    a.name,
                    a.kind,
                    a.flags,
                    a.parent,
                    &a.supers,
                    &a.super_args,
                    &a.annotations,
                ) != (
                    b.name,
                    b.kind,
                    b.flags,
                    b.parent,
                    &b.supers,
                    &b.super_args,
                    &b.annotations,
                )
            });
            let at = first.map(|k| {
                let (a, b) = (&fp.decls[k], &old_decls[k]);
                format!(
                    " at #{k}: {} kind {} flags {:#x} parent {} vs {} kind {} flags {:#x} parent {}",
                    interner.strs[a.name as usize], a.kind, a.flags, a.parent, p.str(b.name), b.kind, b.flags, b.parent
                )
            });
            return bail(&format!(
                "declarations of {} changed ({} vs {}){}",
                entries[i].path,
                fp.decls.len(),
                old_decls.len(),
                at.unwrap_or_default()
            ));
        }
        let before = old_refs(i);
        if !before.koin.is_empty() || !fp.refs.koin.is_empty() {
            return bail(&format!("{} has a Koin module", entries[i].path));
        }
        fresh.insert(i, (fp, before));
    }
    let parse_ms = t1.elapsed().as_millis();

    // ---- relink the changed files against the unchanged symbol table
    let t2 = Instant::now();
    let mut sym = Symbols::default();
    for (i, e) in entries.iter().enumerate() {
        let (m, test) = modules.locate(&e.path);
        match fresh.get(&i) {
            Some((fp, _)) => sym.push_file(fp.package, m, test, &fp.decls),
            None => push_carried(&mut sym, p, i, m, test),
        }
    }
    lap("symbols", t2);
    let linker = Linker::new(&sym, &modules.vis, &[], &interner.strs, |s| interner.get(s));
    lap("linker", t2);
    let mut outs: Vec<(usize, crate::graph::FileOut)> = Vec::with_capacity(fresh.len());
    for (&i, (fp, before)) in &fresh {
        let now = linker.link_file(i, &fp.refs);
        if !now.same_reach_outside(&linker.link_file(i, before)) {
            return bail(&format!(
                "{} links differently past its own pairs",
                entries[i].path
            ));
        }
        outs.push((i, now));
    }
    let declares = p.csr(S_FAMILIES + 2 * Family::Declares as usize);
    let type_args = p.csr(S_TARGS_OFF);
    lap("relink files", t2);
    let prep = linker.prep(&declares, &type_args, |s| interner.get(s));
    lap("prep", t2);
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let (mut refby, mut vblock, mut targ_only, mut via, mut users) =
        (vec![], vec![], vec![], vec![], vec![]);
    for (i, out) in outs {
        let r = sym.file_nodes(i);
        ranges.push((r.start, r.end));
        let c = linker.assemble(i, &fresh[&i].0.refs, out, &prep);
        refby.extend(c.refby);
        vblock.extend(c.vblock);
        targ_only.extend(c.targ_only);
        via.extend(c.via);
        users.extend(c.users);
    }
    lap("assemble", t2);
    let mine = |v: u32| ranges.iter().any(|&(a, b)| a <= v && v < b);
    let n = p.n_nodes();
    let fam = |f: Family| S_FAMILIES + 2 * f as usize;
    let p_refby = patch(
        p.u32s(fam(Family::RefBy)),
        p.u32s(fam(Family::RefBy) + 1),
        n,
        |_, v| mine(v),
        refby,
    );
    let p_vblock = patch(
        p.u32s(fam(Family::VBlock)),
        p.u32s(fam(Family::VBlock) + 1),
        n,
        |_, v| mine(v),
        vblock,
    );
    let p_targ_only = patch(
        p.u32s(fam(Family::TargOnly)),
        p.u32s(fam(Family::TargOnly) + 1),
        n,
        |_, v| mine(v),
        targ_only,
    );
    let p_via = patch(
        p.u32s(fam(Family::TargVia)),
        p.u32s(fam(Family::TargVia) + 1),
        n,
        |r, _| mine(r),
        via,
    );
    let user_rows = p
        .family_rows(Family::NameUsers)
        .max(users.iter().map(|u| u.0 as usize + 1).max().unwrap_or(0));
    let mut p_users = patch(
        p.u32s(fam(Family::NameUsers)),
        p.u32s(fam(Family::NameUsers) + 1),
        user_rows,
        |_, v| mine(v),
        users,
    );
    // A cold build has as many name rows as the highest name with a user.
    while p_users.off.len() > 1
        && p_users.off[p_users.off.len() - 1] == p_users.off[p_users.off.len() - 2]
    {
        p_users.off.pop();
    }
    let link_ms = t2.elapsed().as_millis();

    // ---- parses.bin: append the changed files' records
    let t3 = Instant::now();
    let old_len = parses_old.len() as u64;
    drop(parses_old);
    let mut live = p.meta(M_LIVE);
    let mut buf: Vec<u32> = Vec::new();
    let mut rec: FxHashMap<usize, (u64, u32)> = FxHashMap::default();
    for (&i, (fp, _)) in &fresh {
        let mut w = Vec::new();
        encode_refs(&fp.refs, &mut w);
        rec.insert(i, (old_len + buf.len() as u64 * 4, w.len() as u32));
        live = live - p.file(i)[F_REC_LEN] as u64 * 4 + w.len() as u64 * 4;
        buf.extend(w);
    }
    if old_len + buf.len() as u64 * 4 > 3 * live.max(1) {
        return bail("parses.bin is mostly garbage; the full path rewrites it");
    }
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&paths.parses)?;
        f.write_all(bytemuck::cast_slice::<u32, u8>(&buf))?;
    }

    // ---- graph.bin: the old sections, with the changed files' rows and pairs replaced
    lap("parses append", t3);
    let mut files_tab: Vec<u32> = p.u32s(S_FILES).to_vec();
    for (i, (e, st)) in entries.iter().zip(statuses).enumerate() {
        let row = &mut files_tab[i * FILE_W..(i + 1) * FILE_W];
        let put64 = |row: &mut [u32], at: usize, v: u64| {
            row[at] = v as u32;
            row[at + 1] = (v >> 32) as u32;
        };
        match st {
            Status::Clean(_) => {}
            Status::Restat(_) => {
                put64(row, F_MT, e.mtime);
                put64(row, F_SIZE, e.size);
            }
            Status::Changed(_, t) => {
                put64(row, F_MT, e.mtime);
                put64(row, F_SIZE, e.size);
                put64(row, F_HASH, hash(t.as_bytes()));
                put64(row, F_REC_OFF, rec[&i].0);
                row[F_REC_LEN] = rec[&i].1;
            }
            Status::New(_) => unreachable!(),
        }
    }
    let mut nodes_tab: Vec<u32> = p.u32s(S_NODES).to_vec();
    for (&i, (fp, _)) in &fresh {
        let base = p.file_base(i) as usize;
        for (k, d) in fp.decls.iter().enumerate() {
            nodes_tab[(base + k) * NODE_W + 4] = d.lo;
            nodes_tab[(base + k) * NODE_W + 5] = d.hi;
        }
    }
    let mut blob: Vec<u8> = p.bytes(S_STR_BLOB).to_vec();
    let mut str_off: Vec<u32> = p.u32s(S_STR_OFF).to_vec();
    for s in &interner.strs[p.n_strings()..] {
        blob.extend_from_slice(s.as_bytes());
        str_off.push(blob.len() as u32);
    }
    let n_edges = p_refby.val.len();
    let meta: Vec<u64> = vec![
        FORMAT as u64,
        p.meta(M_STAMP),
        entries.len() as u64,
        n as u64,
        n_edges as u64,
        interner.strs.len() as u64,
        live,
    ];
    let patched: [(Family, &Csr); 5] = [
        (Family::RefBy, &p_refby),
        (Family::VBlock, &p_vblock),
        (Family::TargOnly, &p_targ_only),
        (Family::TargVia, &p_via),
        (Family::NameUsers, &p_users),
    ];
    let mut sections: Vec<&[u8]> = (0..N_SECTIONS).map(|s| p.bytes(s)).collect();
    sections[S_META] = bytemuck::cast_slice(&meta);
    sections[S_STR_BLOB] = &blob;
    sections[S_STR_OFF] = bytemuck::cast_slice(&str_off);
    sections[S_FILES] = bytemuck::cast_slice(&files_tab);
    sections[S_NODES] = bytemuck::cast_slice(&nodes_tab);
    for (f, c) in patched {
        sections[fam(f)] = bytemuck::cast_slice(&c.off);
        sections[fam(f) + 1] = bytemuck::cast_slice(&c.val);
    }
    lap("tables", t3);
    write_packed(&paths.graph, &sections)?;
    let write_ms = t3.elapsed().as_millis();
    Ok(Some(Report {
        mode: "delta",
        files: entries.len(),
        nodes: n,
        edges: n_edges,
        reparsed: fresh.len(),
        relinked: fresh.len(),
        walk_ms,
        parse_ms,
        link_ms,
        write_ms,
        total_ms: t0.elapsed().as_millis(),
    }))
}

/// `off`/`val` rows `0..rows` with the pairs `drop(row, value)` matches removed and `add` merged in,
/// each row kept sorted and deduplicated as `Csr::from_pairs` leaves it.
fn patch(
    off: &[u32],
    val: &[u32],
    rows: usize,
    drop: impl Fn(u32, u32) -> bool,
    mut add: Vec<(u32, u32)>,
) -> Csr {
    add.sort_unstable();
    add.dedup();
    let old_rows = off.len().saturating_sub(1);
    let mut c = Csr {
        off: Vec::with_capacity(rows + 1),
        val: Vec::with_capacity(val.len() + add.len()),
    };
    c.off.push(0);
    let mut j = 0;
    for r in 0..rows {
        let old: &[u32] = if r < old_rows {
            &val[off[r] as usize..off[r + 1] as usize]
        } else {
            &[]
        };
        let start = j;
        while j < add.len() && add[j].0 as usize == r {
            j += 1;
        }
        let new: &[(u32, u32)] = &add[start..j];
        let row = r as u32;
        if new.is_empty() && !old.iter().any(|&v| drop(row, v)) {
            c.val.extend_from_slice(old);
        } else {
            let mut a = old.iter().copied().filter(|&v| !drop(row, v)).peekable();
            let mut b = new.iter().map(|p| p.1).peekable();
            loop {
                let next = match (a.peek().copied(), b.peek().copied()) {
                    (Some(x), Some(y)) if x < y => a.next(),
                    (Some(x), Some(y)) if y < x => b.next(),
                    (Some(_), Some(_)) => {
                        b.next();
                        a.next()
                    }
                    (Some(_), None) => a.next(),
                    (None, Some(_)) => b.next(),
                    (None, None) => break,
                };
                c.val.push(next.unwrap());
            }
        }
        c.off.push(c.val.len() as u32);
    }
    c
}

fn write_packed(path: &Path, sections: &[&[u8]]) -> std::io::Result<()> {
    assert_eq!(sections.len(), N_SECTIONS);
    let header = 16 + N_SECTIONS * 16;
    let mut table: Vec<(u64, u64)> = Vec::with_capacity(N_SECTIONS);
    let mut at = header.next_multiple_of(8) as u64;
    for s in sections {
        table.push((at, s.len() as u64));
        at = (at + s.len() as u64).next_multiple_of(8);
    }
    let tmp = path.with_extension("bin.tmp");
    let mut f = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&tmp)?);
    f.write_all(MAGIC)?;
    f.write_all(&(N_SECTIONS as u64).to_le_bytes())?;
    for (o, l) in &table {
        f.write_all(&o.to_le_bytes())?;
        f.write_all(&l.to_le_bytes())?;
    }
    let mut pos = header as u64;
    for (s, (o, _)) in sections.iter().zip(&table) {
        while pos < *o {
            f.write_all(&[0])?;
            pos += 1;
        }
        f.write_all(s)?;
        pos += s.len() as u64;
    }
    f.flush()?;
    drop(f);
    std::fs::rename(&tmp, path)
}

/// Every (target, source) edge of a graph, for comparing an incremental result with a cold one.
pub fn edge_set(p: &Packed) -> Vec<(String, u32, String, u32)> {
    let mut out = Vec::with_capacity(p.n_edges());
    let file_of = |n: u32| -> (usize, u32) {
        let f = p.node(n as usize)[3] as usize;
        (f, n - p.file_base(f))
    };
    for t in 0..p.n_nodes() as u32 {
        let (tf, tl) = file_of(t);
        for &s in p.referenced_by(t as usize) {
            let (sf, sl) = file_of(s);
            out.push((
                p.file_path(tf).to_string(),
                tl,
                p.file_path(sf).to_string(),
                sl,
            ));
        }
    }
    out.sort();
    out
}

/// Canonical id-free dump of the graph, one `family\ta\tb` line per fact, for diffing two graphs.
pub fn dump_edges(p: &Packed) -> Vec<String> {
    use crate::graph::{N_ANY_MEMBERS, N_IN_TEST, N_MARKER, N_NO_WALK, N_PRIVATE};
    let n = p.n_nodes();
    let mut keys: Vec<String> = Vec::with_capacity(n);
    let mut chain: Vec<String> = Vec::with_capacity(n);
    let mut seen: FxHashMap<(u32, i64, u32), u32> = FxHashMap::default();
    for i in 0..n {
        let row = p.node(i);
        let (name, parent, f) = (row[0], row[2] as i32, row[3]);
        let o = seen.entry((f, parent as i64, name)).or_insert(0);
        let ord = *o;
        *o += 1;
        let base = if parent >= 0 {
            format!("{}/", chain[parent as usize])
        } else {
            String::new()
        };
        let c = format!(
            "{base}{}{}",
            p.str(name),
            if ord > 0 {
                format!("~{ord}")
            } else {
                String::new()
            }
        );
        keys.push(format!("{}#{c}", p.file_path(f as usize)));
        chain.push(c);
    }
    let mut out = Vec::new();
    let flag_names = [
        ("in_test", N_IN_TEST),
        ("no_walk", N_NO_WALK),
        ("private", N_PRIVATE),
        ("marker", N_MARKER),
        ("any_members", N_ANY_MEMBERS),
    ];
    use crate::graph::N_ASKED;
    for i in 0..n {
        out.push(format!("node\t{}\t", keys[i]));
        let row = p.node(i);
        let flags = (row[1] >> 8 & 0xff) as u8;
        if flags & parse::FLAG_TEST_ROOT != 0 {
            let f = row[3] as usize;
            let pkg = p.str(p.file(f)[F_PKG]);
            let name = p.str(row[0]);
            let fq = if pkg.is_empty() {
                name.to_string()
            } else {
                format!("{pkg}.{name}")
            };
            out.push(format!("root_test\t{}\t{fq}", keys[i]));
        }
        let nf = p.node_flags(i);
        for (name, bit) in flag_names {
            if nf & bit != 0 {
                out.push(format!("flag:{name}\t{}\t", keys[i]));
            }
        }
        if nf & N_ASKED != 0 {
            out.push(format!("asked\t{}\t", keys[i]));
        }
        let pr = p.family(Family::Provides, i);
        let mut k = 0;
        while k + 1 < pr.len() {
            let (m, len) = (pr[k] as usize, pr[k + 1] as usize);
            out.push(format!("provides\t{}\t{}", keys[i], keys[m]));
            for &w in &pr[k + 2..k + 2 + len] {
                out.push(format!(
                    "provides_walk\t{}@{}\t{}",
                    keys[i], keys[m], keys[w as usize]
                ));
            }
            k += 2 + len;
        }
        let br = p.family(Family::Builds, i);
        if br == [NONE] {
            out.push(format!("builds_any\t{}\t", keys[i]));
        } else {
            for &x in br {
                out.push(format!("builds\t{}\t{}", keys[i], keys[x as usize]));
            }
        }
        for &m in p.family(Family::KoinRouted, i) {
            out.push(format!("koin_routed\t{}\t{}", keys[i], keys[m as usize]));
        }
        // Row-keyed by target: emit (source, target).
        for (fam, name) in [
            (Family::RefBy, "ref"),
            (Family::VBlock, "vblock"),
            (Family::TargOnly, "targ_only"),
        ] {
            for &s in p.family(fam, i) {
                out.push(format!("{name}\t{}\t{}", keys[s as usize], keys[i]));
            }
        }
        // Row-keyed by source: emit (source, target).
        for (fam, name) in [
            (Family::Declares, "super"),
            (Family::Anon, "anon"),
            (Family::TargVia, "targ_via"),
            (Family::ExtRecv, "ext_recv"),
        ] {
            for &t in p.family(fam, i) {
                out.push(format!("{name}\t{}\t{}", keys[i], keys[t as usize]));
            }
        }
    }
    for w in 0..p.family_rows(Family::NameUsers) {
        for &s in p.family(Family::NameUsers, w) {
            out.push(format!(
                "name_users\t{}\t{}",
                p.str(w as u32),
                keys[s as usize]
            ));
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover;

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("kt-affected-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Writes `text`, stamping a strictly increasing mtime so no edit hides behind an equal stat.
    fn write(root: &Path, rel: &str, text: &str, clock: &mut u64) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        *clock += 1;
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + *clock);
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
            &v[(self.next() % v.len() as u64) as usize]
        }
    }

    const NAMES: &[&str] = &[
        "Repo", "Service", "Handler", "Event", "Base", "Model", "helper", "run", "load", "Payload",
        "Store", "MemStore",
    ];

    fn fixture(root: &Path, clock: &mut u64) -> Vec<String> {
        write(
            root,
            "settings.gradle.kts",
            "rootProject.name = \"demo\"\ninclude(\":core\", \":app\")\n",
            clock,
        );
        write(
            root,
            "core/build.gradle.kts",
            "plugins { kotlin(\"jvm\") }\n",
            clock,
        );
        write(
            root,
            "app/build.gradle.kts",
            "plugins { kotlin(\"jvm\") }\ndependencies {\n  implementation(project(\":core\"))\n}\n",
            clock,
        );
        let files: Vec<(&str, &str)> = vec![
            (
                "core/src/main/kotlin/demo/Base.kt",
                "package demo\n\nabstract class Base<T> {\n    abstract fun handle(e: T)\n    fun load(): T? = null\n}\n\ninterface Store {\n    fun put(x: Any)\n}\n",
            ),
            (
                "core/src/main/kotlin/demo/Event.kt",
                "package demo\n\nsealed class Event {\n    data class Payload(val id: Int) : Event()\n    object Tick : Event()\n}\n\nclass Model(val name: String) {\n    class Nested\n    fun run() = name.length\n}\n",
            ),
            (
                "core/src/main/kotlin/demo/Repo.kt",
                "package demo\n\nclass Repo(private val store: Store) {\n    fun save(m: Model) {\n        store.put(m)\n    }\n    fun helper(): Int = 1\n}\n\nfun Model.describe(): String = name\n",
            ),
            (
                "core/src/main/kotlin/demo/Di.kt",
                "package demo\n\nimport org.koin.dsl.module\n\nval coreModule = module {\n    single { Repo(get()) }\n    single<Store> { MemStore() }\n}\n\nclass MemStore : Store {\n    override fun put(x: Any) {}\n}\n",
            ),
            (
                "app/src/main/kotlin/demo/app/Handler.kt",
                "package demo.app\n\nimport demo.Base\nimport demo.Event\nimport demo.Repo\n\nclass Handler(private val repo: Repo) : Base<Event.Payload>() {\n    override fun handle(e: Event.Payload) {\n        repo.helper()\n        e.id\n    }\n}\n",
            ),
            (
                "app/src/main/kotlin/demo/app/Service.kt",
                "package demo.app\n\nimport demo.Model\nimport demo.describe\n\nclass Service {\n    fun run(m: Model): String {\n        return m.describe()\n    }\n}\n",
            ),
            (
                "app/src/test/kotlin/demo/app/HandlerTest.kt",
                "package demo.app\n\nimport demo.Event\nimport kotlin.test.Test\n\nclass HandlerTest {\n    @Test\n    fun handles() {\n        Event.Payload(1).id\n    }\n}\n",
            ),
            (
                "app/src/test/kotlin/demo/app/ServiceTest.kt",
                "package demo.app\n\nimport demo.Model\nimport kotlin.test.Test\n\nclass ServiceTest {\n    @Test\n    fun runs() {\n        Service().run(Model(\"x\"))\n    }\n}\n",
            ),
        ];
        for (p, t) in &files {
            write(root, p, t, clock);
        }
        files.iter().map(|(p, _)| p.to_string()).collect()
    }

    /// One random edit of one of the kinds an incremental update must get right; returns the kind.
    fn mutate(
        root: &Path,
        files: &mut Vec<String>,
        rng: &mut Rng,
        clock: &mut u64,
        round: usize,
    ) -> &'static str {
        let kinds = [
            "body",
            "body",
            "body",
            "import",
            "newdecl",
            "rename",
            "deldecl",
            "supertype",
            "koin",
            "delete",
            "add",
        ];
        let kind = *rng.pick(&kinds);
        let rel = rng.pick(files).clone();
        let path = root.join(&rel);
        let text = std::fs::read_to_string(&path).unwrap();
        let name = *rng.pick(NAMES);
        let edited = match kind {
            "body" => text.replacen(") {\n", &format!(") {{\n        {name}.toString()\n"), 1),
            "import" => text.replacen("\n\n", &format!("\n\nimport demo.{name}\n"), 1),
            "newdecl" => format!("{text}\nfun probe{round}() = {name}::class\n"),
            "rename" => text.replacen("fun ", &format!("fun r{round}"), 1),
            "deldecl" => match text.rfind("\nfun ") {
                Some(i) => text[..i].to_string() + "\n",
                None => text.clone(),
            },
            "supertype" => text
                .replacen(
                    "class Service {",
                    &format!("class Service : {name}() {{"),
                    1,
                )
                .replacen("class Model(", &format!("class Model : {name}("), 1),
            "koin" => text.replacen(
                "module {\n",
                &format!("module {{\n    single {{ {name}() }}\n"),
                1,
            ),
            "delete" if files.len() > 3 => {
                std::fs::remove_file(&path).unwrap();
                files.retain(|f| f != &rel);
                return kind;
            }
            "add" => {
                let new = format!("app/src/main/kotlin/demo/app/Added{round}.kt");
                write(
                    root,
                    &new,
                    &format!(
                        "package demo.app\n\nclass Added{round} {{\n    fun go() = {name}()\n}}\n"
                    ),
                    clock,
                );
                files.push(new);
                return kind;
            }
            _ => text.clone(),
        };
        write(root, &rel, &edited, clock);
        kind
    }

    #[test]
    fn incremental_updates_equal_a_cold_build() {
        let root = tmp("equiv");
        let mut clock = 0;
        let mut files = fixture(&root, &mut clock);
        let build = discover::discover(&root);
        let (inc, cold) = (root.join("build/inc"), root.join("build/cold"));
        update(&root, &build, &inc, true).unwrap();
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut modes: FxHashMap<&'static str, usize> = FxHashMap::default();
        for round in 0..150 {
            let kind = mutate(&root, &mut files, &mut rng, &mut clock, round);
            let r = update(&root, &build, &inc, false).unwrap();
            *modes.entry(r.mode).or_default() += 1;
            let _ = std::fs::remove_dir_all(&cold);
            update(&root, &build, &cold, true).unwrap();
            let a = dump_edges(&Packed::open(&inc.join("graph.bin")).unwrap());
            let b = dump_edges(&Packed::open(&cold.join("graph.bin")).unwrap());
            if a != b {
                let only_a: Vec<_> = a.iter().filter(|x| !b.contains(x)).take(10).collect();
                let only_b: Vec<_> = b.iter().filter(|x| !a.contains(x)).take(10).collect();
                panic!(
                    "round {round} ({kind}, {}): incremental differs\n only incremental: {only_a:#?}\n only cold: {only_b:#?}",
                    r.mode
                );
            }
        }
        eprintln!("modes over 150 edits: {modes:?}");
        assert!(
            modes.get("delta").copied().unwrap_or(0) >= 10,
            "delta path barely exercised: {modes:?}"
        );
        assert!(
            modes.get("incremental").copied().unwrap_or(0) >= 10,
            "full relink barely exercised: {modes:?}"
        );
    }

    fn walk_both(root: &Path) -> (Vec<(String, u64, u64)>, Vec<(String, u64, u64)>) {
        let key = |v: Vec<Entry>| {
            let mut v: Vec<(String, u64, u64)> =
                v.into_iter().map(|e| (e.path, e.mtime, e.size)).collect();
            v.sort();
            v
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        walk(root, "", &mut a);
        walk_portable(root, "", &mut b);
        (key(a), key(b))
    }

    #[test]
    #[cfg(unix)]
    fn bulk_walk_matches_the_portable_walk() {
        let root = tmp("walk");
        let mut clock = 0;
        write(&root, "a/src/main/kotlin/A.kt", "class A", &mut clock);
        write(
            &root,
            "a/src/main/kotlin/deep/er/B.kt",
            "class B {}",
            &mut clock,
        );
        write(&root, "a/build/generated/C.kt", "class C", &mut clock);
        write(&root, "README.md", "x", &mut clock);
        std::os::unix::fs::symlink(
            root.join("a/src/main/kotlin/A.kt"),
            root.join("a/Linked.kt"),
        )
        .unwrap();
        std::os::unix::fs::symlink(root.join("a/src"), root.join("linked-dir")).unwrap();
        let (bulk, portable) = walk_both(&root);
        assert_eq!(bulk, portable);
        assert_eq!(
            bulk.iter().map(|e| e.0.as_str()).collect::<Vec<_>>(),
            [
                "a/Linked.kt",
                "a/src/main/kotlin/A.kt",
                "a/src/main/kotlin/deep/er/B.kt"
            ]
        );
        // The repository this crate sits in, when it is one: every real-world name and stat.
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        if repo.join("settings.gradle.kts").exists() {
            let (bulk, portable) = walk_both(&repo);
            assert_eq!(bulk.len(), portable.len());
            assert!(
                bulk == portable,
                "bulk and portable walks differ on {}",
                repo.display()
            );
        }
    }

    #[test]
    fn patch_replaces_only_matching_pairs() {
        // rows: 0 -> [1, 5, 9], 1 -> [], 2 -> [5]
        let (off, val) = (vec![0, 3, 3, 4], vec![1, 5, 9, 5]);
        let c = patch(
            &off,
            &val,
            4,
            |_, v| v == 5,
            vec![(1, 7), (0, 4), (3, 2), (0, 9)],
        );
        assert_eq!(c.get(0), &[1, 4, 9]);
        assert_eq!(c.get(1), &[7]);
        assert_eq!(c.get(2), &[] as &[u32]);
        assert_eq!(c.get(3), &[2]);
    }
}

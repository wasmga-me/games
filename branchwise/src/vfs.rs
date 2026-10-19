//! An in-memory file system, served to libgit2 (built for WASI) through the
//! WASI calls it imports, which this module defines, so the game needs
//! nothing from the page. Each world owns one `Fs`; `enter` swaps it in
//! while libgit2 runs.
//!
//! Times come from a counter, not the clock, so replaying the same commands
//! gives the same commits and the same ids.

use std::collections::BTreeMap;

#[derive(Clone)]
enum Kind {
    Dir(BTreeMap<String, usize>),
    File(Vec<u8>),
}

#[derive(Clone)]
struct Node {
    kind: Kind,
    mtime: u64,
}

#[derive(Clone)]
struct Open {
    node: usize,
    pos: u64,
    append: bool,
}

#[derive(Clone)]
pub struct Fs {
    nodes: Vec<Node>,
    fds: BTreeMap<u32, Open>,
    next_fd: u32,
    /// seconds since the epoch, moved on by every clock read
    pub now: u64,
    rand: u64,
}

const ROOT: usize = 0;
/// the one preopened directory, "/"
const PRE: u32 = 3;

// WASI errno values
const EBADF: i32 = 8;
const EEXIST: i32 = 20;
const EINVAL: i32 = 28;
const EISDIR: i32 = 31;
const ENOENT: i32 = 44;
const ENOSYS: i32 = 52;
const ENOTDIR: i32 = 54;
const ENOTEMPTY: i32 = 55;
const ENOTSUP: i32 = 58;

const FT_DIR: u8 = 3;
const FT_FILE: u8 = 4;
const FT_CHAR: u8 = 2;

impl Default for Fs {
    fn default() -> Self {
        Fs {
            nodes: vec![Node {
                kind: Kind::Dir(BTreeMap::new()),
                mtime: 0,
            }],
            fds: BTreeMap::new(),
            next_fd: 4,
            // 2026-01-01
            now: 1_767_225_600,
            rand: 0x9e37_79b9_7f4a_7c15,
        }
    }
}

/// Splits a path into its parts, dropping "." and empty ones.
fn parts(p: &str) -> Vec<&str> {
    p.split('/').filter(|s| !s.is_empty() && *s != ".").collect()
}

impl Fs {
    fn tick(&mut self) -> u64 {
        self.now += 1;
        self.now
    }

    fn dir(&self, n: usize) -> Option<&BTreeMap<String, usize>> {
        match &self.nodes[n].kind {
            Kind::Dir(d) => Some(d),
            _ => None,
        }
    }
    fn dir_mut(&mut self, n: usize) -> Option<&mut BTreeMap<String, usize>> {
        match &mut self.nodes[n].kind {
            Kind::Dir(d) => Some(d),
            _ => None,
        }
    }

    /// Follows `path` from directory `at`; ".." goes up (stopping at "/").
    fn walk(&self, at: usize, path: &str) -> Result<usize, i32> {
        let mut stack = vec![at];
        if path.starts_with('/') {
            stack = vec![ROOT];
        }
        for p in parts(path) {
            if p == ".." {
                if stack.len() > 1 {
                    stack.pop();
                } else if *stack.last().unwrap() != ROOT {
                    // went above the start: find the parent by searching
                    let cur = *stack.last().unwrap();
                    let parent = self.parent_of(cur).unwrap_or(ROOT);
                    stack = vec![parent];
                }
                continue;
            }
            let cur = *stack.last().unwrap();
            let d = self.dir(cur).ok_or(ENOTDIR)?;
            let next = *d.get(p).ok_or(ENOENT)?;
            stack.push(next);
        }
        Ok(*stack.last().unwrap())
    }

    fn parent_of(&self, n: usize) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, x)| matches!(&x.kind, Kind::Dir(d) if d.values().any(|&v| v == n)))
            .map(|(i, _)| i)
    }

    /// The directory holding `path` and the last name in it.
    fn split_last<'a>(&self, at: usize, path: &'a str) -> Result<(usize, &'a str), i32> {
        let t = path.trim_end_matches('/');
        let (dir, name) = match t.rfind('/') {
            Some(i) => (&t[..i + 1], &t[i + 1..]),
            None => ("", t),
        };
        if name.is_empty() || name == "." || name == ".." {
            return Err(EINVAL);
        }
        let d = self.walk(at, dir)?;
        if self.dir(d).is_none() {
            return Err(ENOTDIR);
        }
        Ok((d, name))
    }

    fn add(&mut self, kind: Kind) -> usize {
        let mtime = self.tick();
        self.nodes.push(Node { kind, mtime });
        self.nodes.len() - 1
    }

    fn fd_node(&self, fd: u32) -> Result<usize, i32> {
        if fd == PRE {
            return Ok(ROOT);
        }
        self.fds.get(&fd).map(|o| o.node).ok_or(EBADF)
    }

    // ---------- for the game: absolute paths ----------

    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        match &self.nodes[self.walk(ROOT, path).ok()?].kind {
            Kind::File(b) => Some(b.clone()),
            _ => None,
        }
    }
    pub fn is_dir(&self, path: &str) -> bool {
        self.walk(ROOT, path).is_ok_and(|n| self.dir(n).is_some())
    }
    pub fn exists(&self, path: &str) -> bool {
        self.walk(ROOT, path).is_ok()
    }
    pub fn write(&mut self, path: &str, data: &[u8]) -> Result<(), i32> {
        let (d, name) = self.split_last(ROOT, path)?;
        let name = name.to_string();
        match self.dir(d).unwrap().get(&name).copied() {
            Some(n) => {
                let t = self.tick();
                let node = &mut self.nodes[n];
                match &mut node.kind {
                    Kind::File(b) => {
                        *b = data.to_vec();
                        node.mtime = t;
                    }
                    _ => return Err(EISDIR),
                }
            }
            None => {
                let n = self.add(Kind::File(data.to_vec()));
                self.dir_mut(d).unwrap().insert(name, n);
            }
        }
        Ok(())
    }
    pub fn mkdir_all(&mut self, path: &str) -> Result<(), i32> {
        let mut cur = ROOT;
        for p in parts(path) {
            let next = self.dir(cur).ok_or(ENOTDIR)?.get(p).copied();
            cur = match next {
                Some(n) => n,
                None => {
                    let n = self.add(Kind::Dir(BTreeMap::new()));
                    self.dir_mut(cur).unwrap().insert(p.to_string(), n);
                    n
                }
            };
        }
        if self.dir(cur).is_none() {
            return Err(ENOTDIR);
        }
        Ok(())
    }
    /// Names in a directory, with whether each is a directory.
    pub fn list(&self, path: &str) -> Option<Vec<(String, bool)>> {
        let d = self.dir(self.walk(ROOT, path).ok()?)?;
        Some(
            d.iter()
                .map(|(k, &v)| (k.clone(), self.dir(v).is_some()))
                .collect(),
        )
    }
    /// Removes a file, or a directory with everything in it.
    pub fn remove(&mut self, path: &str) -> Result<(), i32> {
        let (d, name) = self.split_last(ROOT, path)?;
        let name = name.to_string();
        self.dir_mut(d).unwrap().remove(&name).ok_or(ENOENT)?;
        Ok(())
    }
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), i32> {
        self.rename_at(ROOT, from, ROOT, to)
    }

    fn rename_at(&mut self, fa: usize, from: &str, ta: usize, to: &str) -> Result<(), i32> {
        let (d1, n1) = self.split_last(fa, from)?;
        let (d2, n2) = self.split_last(ta, to)?;
        let (n1, n2) = (n1.to_string(), n2.to_string());
        let node = *self.dir(d1).unwrap().get(&n1).ok_or(ENOENT)?;
        if let Some(&old) = self.dir(d2).unwrap().get(&n2) {
            if old == node {
                return Ok(());
            }
            match (self.dir(old), self.dir(node)) {
                (Some(o), Some(_)) if !o.is_empty() => return Err(ENOTEMPTY),
                (Some(_), None) => return Err(EISDIR),
                (None, Some(_)) => return Err(ENOTDIR),
                _ => {}
            }
        }
        self.dir_mut(d1).unwrap().remove(&n1);
        self.dir_mut(d2).unwrap().insert(n2, node);
        Ok(())
    }

    fn stat(&self, n: usize, out: *mut u8) {
        let node = &self.nodes[n];
        let (ft, size) = match &node.kind {
            Kind::Dir(_) => (FT_DIR, 4096),
            Kind::File(b) => (FT_FILE, b.len() as u64),
        };
        unsafe {
            std::ptr::write_bytes(out, 0, 64);
            put64(out, 0, 1);
            put64(out, 8, n as u64 + 1);
            *out.add(16) = ft;
            put64(out, 24, 1);
            put64(out, 32, size);
            let ns = node.mtime * 1_000_000_000;
            put64(out, 40, ns);
            put64(out, 48, ns);
            put64(out, 56, ns);
        }
    }
}

unsafe fn put64(p: *mut u8, off: usize, v: u64) {
    std::ptr::write_unaligned(p.add(off) as *mut u64, v);
}
unsafe fn put32(p: *mut u8, off: usize, v: u32) {
    std::ptr::write_unaligned(p.add(off) as *mut u32, v);
}
unsafe fn get32(p: *const u8, off: usize) -> u32 {
    std::ptr::read_unaligned(p.add(off) as *const u32)
}
unsafe fn path_arg<'a>(p: i32, len: i32) -> &'a str {
    let b = std::slice::from_raw_parts(p as usize as *const u8, len as usize);
    std::str::from_utf8(b).unwrap_or("")
}
unsafe fn iovs<'a>(p: i32, n: i32) -> Vec<&'a mut [u8]> {
    (0..n as usize)
        .map(|i| {
            let base = (p as usize + i * 8) as *const u8;
            let ptr = get32(base, 0) as usize as *mut u8;
            let len = get32(base, 4) as usize;
            std::slice::from_raw_parts_mut(ptr, len)
        })
        .collect()
}

// ---------- the current file system ----------

static mut CUR: Option<Fs> = None;
/// what libgit2 printed to stdout or stderr, for debugging
pub static mut LOG: Vec<u8> = Vec::new();

/// Runs `f` with `fs` as the file system libgit2 sees.
pub fn enter<T>(fs: &mut Fs, f: impl FnOnce() -> T) -> T {
    unsafe {
        let cur = &mut *std::ptr::addr_of_mut!(CUR);
        *cur = Some(std::mem::take(fs));
        let r = f();
        *fs = cur.take().unwrap();
        r
    }
}

/// The file system inside `enter`.
pub fn cur() -> &'static mut Fs {
    unsafe {
        (*std::ptr::addr_of_mut!(CUR))
            .as_mut()
            .expect("no file system entered")
    }
}

fn wrap(f: impl FnOnce(&mut Fs) -> Result<(), i32>) -> i32 {
    match f(cur()) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

macro_rules! wasi {
    ($name:ident ( $($a:ident : $t:ty),* ) $body:block) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($($a: $t),*) -> i32 $body
    };
}

wasi!(__imported_wasi_snapshot_preview1_args_sizes_get(argc: i32, size: i32) {
    put32(argc as usize as *mut u8, 0, 0);
    put32(size as usize as *mut u8, 0, 0);
    0
});
wasi!(__imported_wasi_snapshot_preview1_args_get(_argv: i32, _buf: i32) { 0 });

const ENV: &[u8] = b"HOME=/home/you\0";
wasi!(__imported_wasi_snapshot_preview1_environ_sizes_get(count: i32, size: i32) {
    put32(count as usize as *mut u8, 0, 1);
    put32(size as usize as *mut u8, 0, ENV.len() as u32);
    0
});
wasi!(__imported_wasi_snapshot_preview1_environ_get(env: i32, buf: i32) {
    std::ptr::copy_nonoverlapping(ENV.as_ptr(), buf as usize as *mut u8, ENV.len());
    put32(env as usize as *mut u8, 0, buf as u32);
    0
});

wasi!(__imported_wasi_snapshot_preview1_clock_res_get(_id: i32, out: i32) {
    put64(out as usize as *mut u8, 0, 1_000_000_000);
    0
});
wasi!(__imported_wasi_snapshot_preview1_clock_time_get(_id: i32, _prec: i64, out: i32) {
    let t = match (*std::ptr::addr_of_mut!(CUR)).as_mut() {
        Some(fs) => fs.tick(),
        None => 1_767_225_600,
    };
    put64(out as usize as *mut u8, 0, t * 1_000_000_000);
    0
});
wasi!(__imported_wasi_snapshot_preview1_random_get(buf: i32, len: i32) {
    let fs = cur();
    for i in 0..len as usize {
        fs.rand ^= fs.rand << 13;
        fs.rand ^= fs.rand >> 7;
        fs.rand ^= fs.rand << 17;
        *((buf as usize + i) as *mut u8) = fs.rand as u8;
    }
    0
});
#[no_mangle]
pub unsafe extern "C" fn __imported_wasi_snapshot_preview1_proc_exit(code: i32) {
    panic!("proc_exit {code}");
}
wasi!(__imported_wasi_snapshot_preview1_sched_yield() { 0 });
wasi!(__imported_wasi_snapshot_preview1_poll_oneoff(_a: i32, _b: i32, _n: i32, _out: i32) { ENOSYS });

wasi!(__imported_wasi_snapshot_preview1_fd_prestat_get(fd: i32, out: i32) {
    if fd as u32 != PRE {
        return EBADF;
    }
    let out = out as usize as *mut u8;
    *out = 0;
    put32(out, 4, 1);
    0
});
wasi!(__imported_wasi_snapshot_preview1_fd_prestat_dir_name(fd: i32, buf: i32, _len: i32) {
    if fd as u32 != PRE {
        return EBADF;
    }
    *(buf as usize as *mut u8) = b'/';
    0
});

wasi!(__imported_wasi_snapshot_preview1_fd_fdstat_get(fd: i32, out: i32) {
    let out = out as usize as *mut u8;
    std::ptr::write_bytes(out, 0, 24);
    let ft = if fd < 3 {
        FT_CHAR
    } else {
        let fs = cur();
        match fs.fd_node(fd as u32) {
            Ok(n) => if fs.dir(n).is_some() { FT_DIR } else { FT_FILE },
            Err(e) => return e,
        }
    };
    *out = ft;
    put64(out, 8, u64::MAX);
    put64(out, 16, u64::MAX);
    0
});
wasi!(__imported_wasi_snapshot_preview1_fd_fdstat_set_flags(_fd: i32, _f: i32) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_filestat_get(fd: i32, out: i32) {
    let out = out as usize as *mut u8;
    if fd < 3 {
        std::ptr::write_bytes(out, 0, 64);
        *out.add(16) = FT_CHAR;
        return 0;
    }
    wrap(|fs| {
        let n = fs.fd_node(fd as u32)?;
        fs.stat(n, out);
        Ok(())
    })
});
wasi!(__imported_wasi_snapshot_preview1_fd_filestat_set_size(fd: i32, size: i64) {
    wrap(|fs| {
        let n = fs.fd_node(fd as u32)?;
        let t = fs.tick();
        match &mut fs.nodes[n].kind {
            Kind::File(b) => b.resize(size as usize, 0),
            _ => return Err(EISDIR),
        }
        fs.nodes[n].mtime = t;
        Ok(())
    })
});
wasi!(__imported_wasi_snapshot_preview1_fd_filestat_set_times(_fd: i32, _a: i64, _m: i64, _f: i32) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_sync(_fd: i32) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_datasync(_fd: i32) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_advise(_fd: i32, _o: i64, _l: i64, _a: i32) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_allocate(_fd: i32, _o: i64, _l: i64) { 0 });
wasi!(__imported_wasi_snapshot_preview1_fd_renumber(_a: i32, _b: i32) { ENOTSUP });

wasi!(__imported_wasi_snapshot_preview1_fd_close(fd: i32) {
    let fs = cur();
    if fs.fds.remove(&(fd as u32)).is_some() || (fd as u32) < 3 {
        0
    } else {
        EBADF
    }
});

unsafe fn read_at(fs: &mut Fs, fd: u32, iv: i32, n: i32, at: Option<u64>, out: i32) -> i32 {
    let Some(o) = fs.fds.get(&fd).cloned() else {
        if fd < 3 {
            put32(out as usize as *mut u8, 0, 0);
            return 0;
        }
        return EBADF;
    };
    let Kind::File(b) = &fs.nodes[o.node].kind else {
        return EISDIR;
    };
    let mut pos = at.unwrap_or(o.pos) as usize;
    let mut total = 0;
    for buf in iovs(iv, n) {
        let take = buf.len().min(b.len().saturating_sub(pos));
        buf[..take].copy_from_slice(&b[pos..pos + take]);
        pos += take;
        total += take;
        if take < buf.len() {
            break;
        }
    }
    if at.is_none() {
        fs.fds.get_mut(&fd).unwrap().pos = pos as u64;
    }
    put32(out as usize as *mut u8, 0, total as u32);
    0
}

unsafe fn write_at(fs: &mut Fs, fd: u32, iv: i32, n: i32, at: Option<u64>, out: i32) -> i32 {
    let bufs = iovs(iv, n);
    let total: usize = bufs.iter().map(|b| b.len()).sum();
    if fd < 3 {
        let log = &mut *std::ptr::addr_of_mut!(LOG);
        for b in &bufs {
            log.extend_from_slice(b);
        }
        put32(out as usize as *mut u8, 0, total as u32);
        return 0;
    }
    let Some(o) = fs.fds.get(&fd).cloned() else {
        return EBADF;
    };
    let t = fs.tick();
    let node = &mut fs.nodes[o.node];
    let Kind::File(b) = &mut node.kind else {
        return EISDIR;
    };
    let mut pos = match at {
        Some(p) => p as usize,
        None if o.append => b.len(),
        None => o.pos as usize,
    };
    for buf in bufs {
        if b.len() < pos + buf.len() {
            b.resize(pos + buf.len(), 0);
        }
        b[pos..pos + buf.len()].copy_from_slice(buf);
        pos += buf.len();
    }
    node.mtime = t;
    if at.is_none() {
        fs.fds.get_mut(&fd).unwrap().pos = pos as u64;
    }
    put32(out as usize as *mut u8, 0, total as u32);
    0
}

wasi!(__imported_wasi_snapshot_preview1_fd_read(fd: i32, iv: i32, n: i32, out: i32) {
    read_at(cur(), fd as u32, iv, n, None, out)
});
wasi!(__imported_wasi_snapshot_preview1_fd_pread(fd: i32, iv: i32, n: i32, off: i64, out: i32) {
    read_at(cur(), fd as u32, iv, n, Some(off as u64), out)
});
wasi!(__imported_wasi_snapshot_preview1_fd_write(fd: i32, iv: i32, n: i32, out: i32) {
    match (*std::ptr::addr_of_mut!(CUR)).as_mut() {
        Some(fs) => write_at(fs, fd as u32, iv, n, None, out),
        None => write_at(&mut Fs::default(), fd as u32, iv, n, None, out),
    }
});
wasi!(__imported_wasi_snapshot_preview1_fd_pwrite(fd: i32, iv: i32, n: i32, off: i64, out: i32) {
    write_at(cur(), fd as u32, iv, n, Some(off as u64), out)
});
wasi!(__imported_wasi_snapshot_preview1_fd_seek(fd: i32, off: i64, whence: i32, out: i32) {
    let fs = cur();
    let Some(o) = fs.fds.get(&(fd as u32)).cloned() else {
        return EBADF;
    };
    let len = match &fs.nodes[o.node].kind {
        Kind::File(b) => b.len() as i64,
        _ => 0,
    };
    let base = match whence {
        0 => 0,
        1 => o.pos as i64,
        2 => len,
        _ => return EINVAL,
    };
    let p = base + off;
    if p < 0 {
        return EINVAL;
    }
    fs.fds.get_mut(&(fd as u32)).unwrap().pos = p as u64;
    put64(out as usize as *mut u8, 0, p as u64);
    0
});
wasi!(__imported_wasi_snapshot_preview1_fd_tell(fd: i32, out: i32) {
    let fs = cur();
    match fs.fds.get(&(fd as u32)) {
        Some(o) => {
            put64(out as usize as *mut u8, 0, o.pos);
            0
        }
        None => EBADF,
    }
});

wasi!(__imported_wasi_snapshot_preview1_fd_readdir(fd: i32, buf: i32, len: i32, cookie: i64, used: i32) {
    let fs = cur();
    let n = match fs.fd_node(fd as u32) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let Some(d) = fs.dir(n) else {
        return ENOTDIR;
    };
    let mut entries: Vec<(String, usize)> = vec![(".".into(), n), ("..".into(), n)];
    entries.extend(d.iter().map(|(k, &v)| (k.clone(), v)));
    let mut out: Vec<u8> = vec![];
    for (i, (name, node)) in entries.iter().enumerate().skip(cookie as usize) {
        let ft = if fs.dir(*node).is_some() { FT_DIR } else { FT_FILE };
        out.extend_from_slice(&(i as u64 + 1).to_le_bytes());
        out.extend_from_slice(&(*node as u64 + 1).to_le_bytes());
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(&[ft, 0, 0, 0]);
        out.extend_from_slice(name.as_bytes());
        if out.len() >= len as usize {
            break;
        }
    }
    let k = out.len().min(len as usize);
    std::ptr::copy_nonoverlapping(out.as_ptr(), buf as usize as *mut u8, k);
    put32(used as usize as *mut u8, 0, k as u32);
    0
});

wasi!(__imported_wasi_snapshot_preview1_path_open(fd: i32, _dirflags: i32, p: i32, plen: i32, oflags: i32, _rb: i64, _ri: i64, fdflags: i32, out: i32) {
    let path = path_arg(p, plen);
    let fs = cur();
    let at = match fs.fd_node(fd as u32) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let (creat, directory, excl, trunc) = (oflags & 1 != 0, oflags & 2 != 0, oflags & 4 != 0, oflags & 8 != 0);
    let node = match fs.walk(at, path) {
        Ok(n) => {
            if creat && excl {
                return EEXIST;
            }
            let is_dir = fs.dir(n).is_some();
            if directory && !is_dir {
                return ENOTDIR;
            }
            if path.ends_with('/') && !is_dir {
                return ENOTDIR;
            }
            if trunc {
                if is_dir {
                    return EISDIR;
                }
                let t = fs.tick();
                fs.nodes[n].kind = Kind::File(vec![]);
                fs.nodes[n].mtime = t;
            }
            n
        }
        Err(ENOENT) if creat && !directory => {
            let (d, name) = match fs.split_last(at, path) {
                Ok(x) => x,
                Err(e) => return e,
            };
            let name = name.to_string();
            let n = fs.add(Kind::File(vec![]));
            fs.dir_mut(d).unwrap().insert(name, n);
            n
        }
        Err(e) => return e,
    };
    let fdn = fs.next_fd;
    fs.next_fd += 1;
    fs.fds.insert(fdn, Open { node, pos: 0, append: fdflags & 1 != 0 });
    put32(out as usize as *mut u8, 0, fdn);
    0
});

wasi!(__imported_wasi_snapshot_preview1_path_filestat_get(fd: i32, _flags: i32, p: i32, plen: i32, out: i32) {
    let path = path_arg(p, plen);
    wrap(|fs| {
        let at = fs.fd_node(fd as u32)?;
        let n = fs.walk(at, path)?;
        if path.ends_with('/') && fs.dir(n).is_none() {
            return Err(ENOTDIR);
        }
        fs.stat(n, out as usize as *mut u8);
        Ok(())
    })
});
// libgit2 "freshens" a loose object by touching it, and takes success to
// mean the object is there
wasi!(__imported_wasi_snapshot_preview1_path_filestat_set_times(fd: i32, _f: i32, p: i32, plen: i32, _a: i64, _m: i64, _ff: i32) {
    let path = path_arg(p, plen);
    wrap(|fs| {
        let at = fs.fd_node(fd as u32)?;
        let n = fs.walk(at, path)?;
        fs.nodes[n].mtime = fs.now;
        Ok(())
    })
});

wasi!(__imported_wasi_snapshot_preview1_path_create_directory(fd: i32, p: i32, plen: i32) {
    let path = path_arg(p, plen);
    wrap(|fs| {
        let at = fs.fd_node(fd as u32)?;
        if fs.walk(at, path).is_ok() {
            return Err(EEXIST);
        }
        let (d, name) = fs.split_last(at, path)?;
        let name = name.to_string();
        let n = fs.add(Kind::Dir(BTreeMap::new()));
        fs.dir_mut(d).unwrap().insert(name, n);
        Ok(())
    })
});
wasi!(__imported_wasi_snapshot_preview1_path_remove_directory(fd: i32, p: i32, plen: i32) {
    let path = path_arg(p, plen);
    wrap(|fs| {
        let at = fs.fd_node(fd as u32)?;
        let n = fs.walk(at, path)?;
        match fs.dir(n) {
            None => return Err(ENOTDIR),
            Some(d) if !d.is_empty() => return Err(ENOTEMPTY),
            _ => {}
        }
        let (d, name) = fs.split_last(at, path)?;
        let name = name.to_string();
        fs.dir_mut(d).unwrap().remove(&name);
        Ok(())
    })
});
wasi!(__imported_wasi_snapshot_preview1_path_unlink_file(fd: i32, p: i32, plen: i32) {
    let path = path_arg(p, plen);
    wrap(|fs| {
        let at = fs.fd_node(fd as u32)?;
        let n = fs.walk(at, path)?;
        if fs.dir(n).is_some() {
            return Err(EISDIR);
        }
        let (d, name) = fs.split_last(at, path)?;
        let name = name.to_string();
        fs.dir_mut(d).unwrap().remove(&name);
        Ok(())
    })
});
wasi!(__imported_wasi_snapshot_preview1_path_rename(fd: i32, p: i32, plen: i32, nfd: i32, np: i32, nplen: i32) {
    let (from, to) = (path_arg(p, plen), path_arg(np, nplen));
    wrap(|fs| {
        let a = fs.fd_node(fd as u32)?;
        let b = fs.fd_node(nfd as u32)?;
        fs.rename_at(a, from, b, to)
    })
});
wasi!(__imported_wasi_snapshot_preview1_path_link(_a: i32, _f: i32, _p: i32, _l: i32, _b: i32, _q: i32, _m: i32) { ENOTSUP });
wasi!(__imported_wasi_snapshot_preview1_path_symlink(_p: i32, _l: i32, _fd: i32, _q: i32, _m: i32) { ENOTSUP });
wasi!(__imported_wasi_snapshot_preview1_path_readlink(_fd: i32, _p: i32, _l: i32, _b: i32, _bl: i32, _u: i32) { EINVAL });


impl Fs {
    /// Every file under `path` (full paths), skipping .git folders.
    pub fn files_under(&self, path: &str) -> Vec<(String, Vec<u8>)> {
        let mut out = vec![];
        if let Ok(n) = self.walk(ROOT, path) {
            self.collect(n, path.trim_end_matches('/'), false, &mut out);
        }
        out
    }
    fn collect(&self, n: usize, at: &str, git: bool, out: &mut Vec<(String, Vec<u8>)>) {
        match &self.nodes[n].kind {
            Kind::File(b) => out.push((at.to_string(), b.clone())),
            Kind::Dir(d) => {
                for (k, &v) in d {
                    if k == ".git" && !git {
                        continue;
                    }
                    self.collect(v, &format!("{at}/{k}"), git, out);
                }
            }
        }
    }

    /// Changes whenever files, refs or settings do; not for the object
    /// store, logs, the index file (its stat cache) or lock files.
    pub fn fingerprint(&self) -> u64 {
        let mut all = vec![];
        self.collect(ROOT, "", true, &mut all);
        let mut h: u64 = 1469598103934665603;
        for (p, b) in all {
            let skip = p.contains("/objects/")
                || p.contains("/logs/")
                || p.ends_with("/index")
                || p.ends_with(".lock")
                || p.ends_with("FETCH_HEAD")
                || p.ends_with("ORIG_HEAD");
            if skip {
                continue;
            }
            for x in p.bytes().chain([0xff]).chain(b.iter().copied()).chain([0xfe]) {
                h ^= x as u64;
                h = h.wrapping_mul(1099511628211);
            }
        }
        h
    }
}

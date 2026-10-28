//! The world a lesson plays in: real repositories (libgit2) on an
//! in-memory file system, a shell for the commands around git, and a view
//! of the repositories for drawing.
//!
//! Nothing here is saved: a level is its setup commands plus the commands
//! the player ran, and replaying them gives the same files, commits and ids
//! (times come from a counter).

use crate::lg::{self, Oid, Repo as G};
use crate::vfs::{self, Fs};
use std::collections::{HashMap, HashSet};

/// Output line kinds.
pub const OUT: u8 = 0;
pub const ERR: u8 = 1;
pub const GOOD: u8 = 2;
pub const HINT: u8 = 3;
pub const CMD: u8 = 4;
/// a failing test or a removed line: red, but not an error
pub const FAIL: u8 = 5;

pub const HOME: &str = "/home/you";
pub const APP: &str = "/home/you/app";
pub const ORIGIN: &str = "/srv/origin.git";
/// a teammate's clone of origin, for setup commands that stand for their work
pub const SAM: &str = "/home/sam/app";

pub type R<T> = Result<T, String>;

// ---------- the view, for drawing ----------

#[derive(Clone)]
pub struct Commit {
    /// the shortest unique id, 4 characters or more
    pub id: String,
    pub oid: Oid,
    pub parents: Vec<usize>,
    pub msg: String,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Head {
    Branch(String),
    Detached(usize),
}

#[derive(Clone)]
pub struct Worktree {
    /// the folder's name ("app")
    pub path: String,
    pub head: Head,
}

/// One repository as drawn: every commit it has (oldest first), and names.
#[derive(Clone, Default)]
pub struct Repo {
    pub commits: Vec<Commit>,
    pub branches: Vec<(String, usize)>,
    pub tags: Vec<(String, usize)>,
    /// remote-tracking branches, "origin/main"
    pub remote: Vec<(String, usize)>,
    /// local branch, and the branch on origin it tracks ("origin/main")
    pub upstream: Vec<(String, String)>,
    pub wts: Vec<Worktree>,
    pub cur: usize,
}

#[derive(Clone)]
pub struct File {
    pub name: String,
    pub tracked: bool,
    pub staged: bool,
    pub modified: bool,
    pub conflict: bool,
}

#[derive(Clone)]
pub struct Bisect {
    pub good: Vec<usize>,
    pub bad: Option<usize>,
    pub found: Option<usize>,
}

/// An interactive rebase's to-do list, open in the editor.
#[derive(Clone)]
pub struct Todo {
    /// the command as typed, without the list
    pub cmd: String,
    /// (commit, action) with actions b'p' pick, b's' squash, b'f' fixup, b'd' drop;
    /// the number is the line's place in the list as Git first wrote it
    pub lines: Vec<(usize, u8, usize)>,
    pub sel: usize,
}

impl Repo {
    pub fn branch(&self, name: &str) -> Option<usize> {
        self.branches.iter().find(|b| b.0 == name).map(|b| b.1)
    }
    pub fn head_of(&self, h: &Head) -> Option<usize> {
        match h {
            Head::Branch(b) => self.branch(b),
            Head::Detached(c) => Some(*c),
        }
    }
    /// The worktree that has a branch checked out.
    pub fn checked_out(&self, branch: &str) -> Option<usize> {
        self.wts
            .iter()
            .position(|w| w.head == Head::Branch(branch.to_string()))
    }
    pub fn ancestors(&self, c: usize) -> Vec<bool> {
        let mut seen = vec![false; self.commits.len()];
        let mut stack = vec![c];
        while let Some(x) = stack.pop() {
            if !seen[x] {
                seen[x] = true;
                stack.extend(self.commits[x].parents.iter().copied());
            }
        }
        seen
    }
    pub fn index_of(&self, o: &Oid) -> Option<usize> {
        self.commits.iter().position(|c| c.oid == *o)
    }
}

// ---------- what the goal is checked against ----------

#[derive(PartialEq, Debug, Clone, Default)]
pub struct Snap {
    inited: bool,
    branches: Vec<(String, u64)>,
    tags: Vec<(String, u64)>,
    remote: Vec<(String, u64)>,
    upstream: Vec<(String, String)>,
    heads: Vec<(String, String)>,
    cur: String,
    origin: Vec<(String, u64)>,
    origin_tags: Vec<(String, u64)>,
    files: Vec<(String, u8)>,
    stash: usize,
    /// a merge, rebase, cherry-pick or bisect under way
    busy: u8,
}

// ---------- operations that stop half way ----------

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum SeqKind {
    Rebase,
    Pick,
    Revert,
}

/// A rebase, cherry-pick or revert of several commits, which can stop on a
/// conflict and go on with --continue.
#[derive(Clone)]
pub(crate) struct Seq {
    pub kind: SeqKind,
    pub steps: Vec<(u8, Oid)>,
    pub at: usize,
    pub onto: Oid,
    pub orig: Oid,
    pub branch: Option<String>,
    /// branches inside the rebased commits, moved with them (--update-refs)
    pub update_refs: Vec<(String, Oid)>,
    /// old commit -> its copy
    pub rewritten: Vec<(Oid, Oid)>,
    pub stopped: bool,
    pub mainline: u32,
}

#[derive(Clone)]
pub(crate) struct BisectState {
    pub good: Vec<Oid>,
    pub bad: Option<Oid>,
    pub skip: Vec<Oid>,
    /// the branch (or commit) to go back to
    pub orig: String,
    pub found: Option<Oid>,
}

#[derive(Clone)]
pub struct World {
    pub fs: Fs,
    pub cwd: String,
    pub inited: bool,
    pub repo: Repo,
    pub origin: Option<Repo>,
    pub files: Vec<File>,
    pub stash: Vec<String>,
    pub bisect: Option<Bisect>,
    pub todo: Option<Todo>,
    pub out: Vec<(u8, String)>,
    pub(crate) seq: Option<Seq>,
    pub(crate) bis: Option<BisectState>,
    pub(crate) edits: u32,
    /// lines found in the setup, by hash: the goal compares which of them a
    /// commit holds, and counts the others
    known: Option<HashSet<u64>>,
    sigs: HashMap<Oid, u64>,
    snap: Snap,
    pub(crate) quiet: bool,
    /// the shell is inside a repository with a working tree
    pub in_repo: bool,
}

/// Splits a line into commands (on && and ;) and words, with quotes; >
/// and >> are words of their own.
pub fn split(line: &str) -> Vec<Vec<String>> {
    let mut cmds = vec![];
    let mut words: Vec<String> = vec![];
    let mut word = String::new();
    let mut has = false;
    let mut quote: Option<char> = None;
    let cs: Vec<char> = line.chars().collect();
    let mut i = 0;
    let flush = |word: &mut String, has: &mut bool, words: &mut Vec<String>| {
        if *has {
            words.push(std::mem::take(word));
            *has = false;
        }
    };
    while i < cs.len() {
        let c = cs[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                word.push(c);
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
            has = true;
        } else if c == ' ' || c == '\t' {
            flush(&mut word, &mut has, &mut words);
        } else if c == '>' {
            flush(&mut word, &mut has, &mut words);
            if cs.get(i + 1) == Some(&'>') {
                words.push(">>".into());
                i += 1;
            } else {
                words.push(">".into());
            }
        } else if c == ';' || (c == '&' && cs.get(i + 1) == Some(&'&')) {
            flush(&mut word, &mut has, &mut words);
            if !words.is_empty() {
                cmds.push(std::mem::take(&mut words));
            }
            if c == '&' {
                i += 1;
            }
        } else {
            word.push(c);
            has = true;
        }
        i += 1;
    }
    flush(&mut word, &mut has, &mut words);
    if !words.is_empty() {
        cmds.push(words);
    }
    cmds
}

pub(crate) fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
fn mix(a: u64, b: u64) -> u64 {
    let mut h = a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^ (h >> 32)
}

/// Joins `p` onto `base`, folding . and .. ("~" is home).
pub fn join(base: &str, p: &str) -> String {
    let full = if p.starts_with('/') {
        p.to_string()
    } else if p == "~" || p.starts_with("~/") {
        format!("{HOME}{}", &p[1..])
    } else {
        format!("{base}/{p}")
    };
    let mut parts: Vec<&str> = vec![];
    for s in full.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(s),
        }
    }
    format!("/{}", parts.join("/"))
}

/// "/home/you/app" -> "~/app"
pub fn pretty(p: &str) -> String {
    if p == HOME {
        "~".into()
    } else if let Some(r) = p.strip_prefix(&format!("{HOME}/")) {
        format!("~/{r}")
    } else {
        p.to_string()
    }
}

impl World {
    pub fn new(_seed: u32, files: &[&str]) -> World {
        let mut w = World {
            fs: Fs::default(),
            cwd: APP.into(),
            inited: false,
            repo: Repo::default(),
            origin: None,
            files: vec![],
            stash: vec![],
            bisect: None,
            todo: None,
            out: vec![],
            seq: None,
            bis: None,
            edits: 0,
            known: None,
            sigs: HashMap::new(),
            snap: Snap::default(),
            quiet: false,
            in_repo: false,
        };
        w.with_fs(|w| {
            lg::init();
            let f = vfs::cur();
            f.mkdir_all(APP).unwrap();
            f.write(
                &format!("{HOME}/.gitconfig"),
                b"[user]\n\tname = You\n\temail = you@example.com\n[init]\n\tdefaultBranch = main\n[core]\n\tabbrev = 4\n",
            )
            .unwrap();
            for name in files {
                f.write(&format!("{APP}/{name}"), first_lines(name).as_bytes()).unwrap();
            }
            w.refresh();
        });
        w
    }

    /// What the goal compares.
    pub fn snap(&self) -> Snap {
        self.snap.clone()
    }

    /// After a level's setup: every line in it is known from now on.
    pub fn freeze(&mut self) {
        self.with_fs(|w| {
            let mut known = HashSet::new();
            let mut add = |b: &[u8]| {
                for l in String::from_utf8_lossy(b).lines() {
                    known.insert(fnv(l.as_bytes()));
                }
            };
            for dir in [APP, ORIGIN, SAM] {
                if let Ok(g) = lg::open(dir) {
                    for (k, o) in g.objects() {
                        if k == 'b' {
                            if let Some(b) = g.blob(&o) {
                                add(&b);
                            }
                        }
                    }
                }
            }
            for (_, b) in vfs::cur().files_under(HOME) {
                add(&b);
            }
            w.known = Some(known);
            w.sigs.clear();
            w.refresh();
        });
        self.out.clear();
    }

    pub(crate) fn say(&mut self, kind: u8, s: impl Into<String>) {
        if self.quiet {
            return;
        }
        let s = s.into();
        let s = s.strip_suffix('\n').unwrap_or(&s);
        for line in s.split('\n') {
            self.out.push((kind, line.to_string()));
        }
    }
    pub(crate) fn err(&mut self, s: impl Into<String>) {
        self.say(ERR, s);
    }
    pub(crate) fn hint(&mut self, s: impl Into<String>) {
        self.say(HINT, s);
    }

    /// Runs a typed line; true when it changed anything (and so is kept
    /// for the save and undo).
    pub fn run(&mut self, line: &str) -> bool {
        self.with_fs(|w| {
            let before = w.fingerprint();
            for words in split(line) {
                let r = w.exec(&words);
                if let Err(e) = r {
                    if !e.is_empty() {
                        w.err(e);
                    }
                    break;
                }
            }
            let changed = w.fingerprint() != before;
            w.refresh();
            changed
        })
    }

    /// Runs `f` with the world's files as the file system libgit2 sees.
    pub(crate) fn with_fs<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut fs = std::mem::take(&mut self.fs);
        let r = vfs::enter(&mut fs, || f(self));
        self.fs = fs;
        r
    }

    /// Changes whenever the state does (not the output).
    fn fingerprint(&self) -> u64 {
        let mut h = vfs::cur().fingerprint();
        h = mix(h, fnv(self.cwd.as_bytes()));
        for dir in self.worktree_dirs() {
            if let Ok(g) = lg::open(&dir) {
                for (st, mode, id, p) in g.index_entries() {
                    h = mix(h, fnv(format!("{st}{mode}{}{p}", id.hex()).as_bytes()));
                }
            }
        }
        if let Some(s) = &self.seq {
            h = mix(h, (s.at as u64) << 8 | s.stopped as u64 | 1 << 40);
        }
        if let Some(b) = &self.bis {
            h = mix(h, (b.good.len() as u64) << 16 | b.bad.is_some() as u64 | (b.skip.len() as u64) << 24 | 1 << 41);
        }
        h
    }

    fn worktree_dirs(&self) -> Vec<String> {
        let mut v = vec![APP.to_string()];
        if let Ok(g) = lg::open(APP) {
            v.extend(g.worktrees().into_iter().map(|w| w.1));
        }
        v
    }

    // ---------- the view ----------

    /// Rebuilds the view (and the goal snapshot) from the repositories.
    fn refresh(&mut self) {
        let fs = vfs::cur();
        self.inited = fs.exists(&format!("{APP}/.git"));
        let main = if self.inited { lg::open(APP).ok() } else { None };
        self.repo = match &main {
            Some(g) => self.view(g, false),
            None => Repo {
                wts: vec![Worktree {
                    path: "app".into(),
                    head: Head::Branch("main".into()),
                }],
                ..Default::default()
            },
        };
        self.origin = lg::open(ORIGIN).ok().map(|g| self.view(&g, true));
        // the files of the worktree you are in
        self.files.clear();
        self.stash.clear();
        let here = lg::discover(&self.cwd).filter(|g| !g.is_bare());
        self.in_repo = here.is_some();
        if let Some(g) = &here {
            for (f, p) in g.status() {
                let staged = f & 0x1f != 0;
                let modified = f & (lg::ST_WT_NEW | lg::ST_WT_MODIFIED | lg::ST_WT_DELETED | 1024) != 0;
                self.files.push(File {
                    name: p,
                    tracked: f != lg::ST_WT_NEW,
                    staged,
                    modified,
                    conflict: f & lg::ST_CONFLICTED != 0,
                });
            }
        }
        if let Some(g) = &main {
            self.stash = g.stashes().into_iter().map(|s| s.2).collect();
        }
        self.bisect = self.bis.as_ref().map(|b| Bisect {
            good: b.good.iter().filter_map(|o| self.repo.index_of(o)).collect(),
            bad: b.bad.and_then(|o| self.repo.index_of(&o)),
            found: b.found.and_then(|o| self.repo.index_of(&o)),
        });
        if self.known.is_some() {
            self.snap = self.make_snap(main.as_ref());
        }
    }

    fn view(&self, g: &G, bare: bool) -> Repo {
        let objs = g.objects();
        // stash entries are commits too, but not ones to draw
        let mut hide: HashSet<Oid> = HashSet::new();
        for (_, o, _) in g.stashes() {
            if let Ok(c) = g.commit(&o) {
                hide.insert(o);
                hide.extend(c.parents.iter().skip(1).copied());
            }
        }
        let mut infos: Vec<lg::Commit> = objs
            .iter()
            .filter(|(k, o)| *k == 'c' && !hide.contains(o))
            .filter_map(|(_, o)| g.commit(o).ok())
            .collect();
        infos.sort_by_key(|c| (c.ctime, c.id));
        // parents before children
        let mut order: Vec<usize> = vec![];
        let mut placed: HashSet<Oid> = HashSet::new();
        let ids: HashSet<Oid> = infos.iter().map(|c| c.id).collect();
        while order.len() < infos.len() {
            let before = order.len();
            for (i, c) in infos.iter().enumerate() {
                if !placed.contains(&c.id) && c.parents.iter().all(|p| placed.contains(p) || !ids.contains(p)) {
                    placed.insert(c.id);
                    order.push(i);
                }
            }
            if order.len() == before {
                break;
            }
        }
        let all: Vec<Oid> = objs.iter().map(|x| x.1).collect();
        let mut r = Repo::default();
        let mut at: HashMap<Oid, usize> = HashMap::new();
        for &i in &order {
            let c = &infos[i];
            at.insert(c.id, r.commits.len());
            r.commits.push(Commit {
                id: short_among(&c.id, &all),
                oid: c.id,
                parents: vec![],
                msg: c.summary.clone(),
            });
        }
        for &i in &order {
            let c = &infos[i];
            let k = at[&c.id];
            r.commits[k].parents = c.parents.iter().filter_map(|p| at.get(p).copied()).collect();
        }
        for name in g.refs() {
            let Ok(o) = g.commit_of(&name) else { continue };
            let Some(&k) = at.get(&o) else { continue };
            if let Some(b) = name.strip_prefix("refs/heads/") {
                r.branches.push((b.to_string(), k));
            } else if let Some(t) = name.strip_prefix("refs/tags/") {
                r.tags.push((t.to_string(), k));
            } else if let Some(x) = name.strip_prefix("refs/remotes/") {
                if !x.ends_with("/HEAD") {
                    r.remote.push((x.to_string(), k));
                }
            }
        }
        for (b, _) in r.branches.clone() {
            if let Some(u) = g.upstream(&b) {
                r.upstream.push((b, u.trim_start_matches("refs/remotes/").to_string()));
            }
        }
        let head = |g: &G| match (g.detached(), g.head_branch(), g.head()) {
            (true, _, Some(o)) => at.get(&o).map(|&k| Head::Detached(k)),
            (_, Some(b), _) => Some(Head::Branch(b)),
            _ => None,
        };
        if bare {
            r.wts.push(Worktree {
                path: "origin".into(),
                head: head(g).unwrap_or(Head::Branch("main".into())),
            });
            return r;
        }
        let dir = g.workdir();
        r.wts.push(Worktree {
            path: base_name(&dir),
            head: head(g).unwrap_or(Head::Branch("main".into())),
        });
        let mut dirs = vec![dir];
        for (_, p) in g.worktrees() {
            if let Ok(w) = lg::open(&p) {
                if let Some(h) = head(&w) {
                    r.wts.push(Worktree { path: base_name(&p), head: h });
                    dirs.push(p);
                }
            }
        }
        r.cur = dirs
            .iter()
            .position(|d| self.cwd == *d || self.cwd.starts_with(&format!("{d}/")))
            .unwrap_or(0);
        r
    }

    fn tree_sig(&mut self, g: &G, tree: &Oid) -> u64 {
        if let Some(&s) = self.sigs.get(tree) {
            return s;
        }
        let known = self.known.as_ref().unwrap();
        let mut keep: Vec<(String, u64)> = vec![];
        let mut other = 0u64;
        for (blob, path) in g.tree_files(tree) {
            let b = g.blob(&blob).unwrap_or_default();
            for l in String::from_utf8_lossy(&b).lines() {
                let h = fnv(l.as_bytes());
                if known.contains(&h) {
                    keep.push((path.clone(), h));
                } else {
                    other += 1;
                }
            }
        }
        keep.sort();
        let mut s = mix(0x5eed, other);
        for (p, h) in keep {
            s = mix(mix(s, fnv(p.as_bytes())), h);
        }
        self.sigs.insert(*tree, s);
        s
    }

    /// A commit's signature: what its files hold (known lines, and how many
    /// others) and its parents' signatures, so ids and messages don't count.
    fn commit_sigs(&mut self, g: &G, r: &Repo) -> Vec<u64> {
        let mut out: Vec<u64> = vec![];
        for c in &r.commits {
            let tree = g.commit(&c.oid).map(|k| k.tree).unwrap_or_default();
            let mut s = self.tree_sig(g, &tree);
            for &p in &c.parents {
                s = mix(s, out[p]);
            }
            s = mix(s, c.parents.len() as u64);
            out.push(s);
        }
        out
    }

    fn make_snap(&mut self, main: Option<&G>) -> Snap {
        let mut snap = Snap {
            inited: self.inited,
            ..Default::default()
        };
        let refs = |list: &[(String, usize)], sig: &[u64]| {
            let mut v: Vec<(String, u64)> = list.iter().map(|(n, c)| (n.clone(), sig[*c])).collect();
            v.sort();
            v
        };
        if let Some(g) = main {
            let r = self.repo.clone();
            let sig = self.commit_sigs(g, &r);
            snap.branches = refs(&r.branches, &sig);
            snap.tags = refs(&r.tags, &sig);
            snap.remote = refs(&r.remote, &sig);
            snap.upstream = r.upstream.clone();
            snap.upstream.sort();
            snap.heads = r
                .wts
                .iter()
                .map(|w| {
                    let h = match &w.head {
                        Head::Branch(b) => format!("b:{b}"),
                        Head::Detached(c) => format!("c:{}", sig[*c]),
                    };
                    (w.path.clone(), h)
                })
                .collect();
            snap.heads.sort();
            snap.busy = (g.state() != lg::STATE_NONE) as u8;
        }
        if let (Some(o), Ok(g)) = (self.origin.clone(), lg::open(ORIGIN)) {
            let sig = self.commit_sigs(&g, &o);
            snap.origin = refs(&o.branches, &sig);
            snap.origin_tags = refs(&o.tags, &sig);
        }
        snap.cur = pretty(&self.cwd);
        snap.files = self
            .files
            .iter()
            .map(|f| {
                let k = f.staged as u8 | (f.modified as u8) << 1 | (!f.tracked as u8) << 2 | (f.conflict as u8) << 3;
                (f.name.clone(), k)
            })
            .collect();
        snap.files.sort();
        snap.stash = self.stash.len();
        snap.busy |= (self.seq.is_some() as u8) << 1 | (self.bis.is_some() as u8) << 2;
        snap
    }

    // ---------- commands ----------

    fn exec(&mut self, w: &[String]) -> R<()> {
        let args: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
        match args[0] {
            "git" => self.git(&args[1..]),
            "#c" => self.setup_commit(&args[1..]),
            "#clone" => self.setup_clone(),
            "@origin" => self.setup_origin(&w[1..]),
            _ => self.shell(&args),
        }
    }

    /// `#c file message [| line]`: a commit that adds the line (or the
    /// message) to file.
    fn setup_commit(&mut self, a: &[&str]) -> R<()> {
        let words = a[1..].join(" ");
        let (msg, line) = match words.split_once(" | ") {
            Some((m, l)) => (m.to_string(), l.to_string()),
            None => (words.clone(), words.clone()),
        };
        let file = a[0];
        let path = join(&self.cwd, file);
        let fs = vfs::cur();
        let mut b = fs.read(&path).unwrap_or_default();
        b.extend_from_slice(format!("{line}\n").as_bytes());
        if let Some(d) = path.rfind('/') {
            fs.mkdir_all(&path[..d]).map_err(|e| format!("mkdir {e}"))?;
        }
        fs.write(&path, &b).map_err(|e| format!("write {e}"))?;
        let q = self.quiet;
        self.quiet = true;
        let r = self
            .git(&["add", file])
            .and_then(|_| self.git(&["commit", "-q", "-m", &msg]));
        self.quiet = q;
        r
    }

    /// `#clone`: origin becomes a bare copy of app, which tracks it; a
    /// teammate gets a clone of their own.
    fn setup_clone(&mut self) -> R<()> {
        lg::clone(APP, ORIGIN, true)?;
        let g = lg::open(APP)?;
        g.add_remote("origin", ORIGIN)?;
        g.fetch("origin", false, false)?;
        if let Some(b) = g.head_branch() {
            g.set_upstream(&b, Some(&format!("origin/{b}")))?;
        }
        vfs::cur().mkdir_all("/home/sam").map_err(|e| format!("mkdir {e}"))?;
        lg::clone(ORIGIN, SAM, false)?;
        let t = lg::open(SAM)?;
        t.set_config("user.name", Some("Sam"))?;
        t.set_config("user.email", Some("sam@example.com"))?;
        Ok(())
    }

    /// `@origin cmd`: the teammate catches up with origin, runs cmd, and
    /// pushes.
    fn setup_origin(&mut self, cmd: &[String]) -> R<()> {
        let cwd = std::mem::replace(&mut self.cwd, SAM.into());
        let q = self.quiet;
        self.quiet = true;
        let r = (|| {
            let t = lg::open(SAM)?;
            t.fetch("origin", true, true)?;
            let b = t.head_branch().unwrap_or("main".into());
            if let Some(o) = t.ref_id(&format!("refs/remotes/origin/{b}")) {
                t.reset(&o, 3)?;
            }
            drop(t);
            self.exec(cmd)?;
            let t = lg::open(SAM)?;
            let bad = t.push("origin", &[format!("+refs/heads/{b}:refs/heads/{b}")])?;
            if !bad.is_empty() {
                return Err(format!("teammate push: {bad:?}"));
            }
            Ok(())
        })();
        self.quiet = q;
        self.cwd = cwd;
        r
    }

    /// Applies the to-do list in the editor (the rebase runs as one typed
    /// command, so a save replays it).
    pub fn todo_line(&self) -> Option<String> {
        let t = self.todo.as_ref()?;
        let list: Vec<String> = t
            .lines
            .iter()
            .map(|&(_, act, k)| format!("{}{k}", act as char))
            .collect();
        Some(format!("{} #todo {}", t.cmd, list.join(" ")))
    }

    /// The prompt's folder ("~/app").
    pub fn where_label(&self) -> String {
        pretty(&self.cwd)
    }
}

/// What a new file holds before the lesson starts.
fn first_lines(name: &str) -> String {
    match name {
        "README.md" => "# app\n\nA small web app.\n".into(),
        ".env" => "PASSWORD=hunter2\n".into(),
        _ => format!("{name}, first version\n"),
    }
}

pub(crate) fn base_name(p: &str) -> String {
    p.trim_end_matches('/').rsplit('/').next().unwrap_or(p).to_string()
}

/// The shortest prefix of `id` (4 or more) no other object shares.
pub(crate) fn short_among(id: &Oid, all: &[Oid]) -> String {
    let h = id.hex();
    let others: Vec<String> = all.iter().filter(|o| *o != id).map(|o| o.hex()).collect();
    for n in 4..40 {
        let p = &h[..n];
        if !others.iter().any(|o| o.starts_with(p)) {
            return p.to_string();
        }
    }
    h
}

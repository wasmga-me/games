//! A small git: commits, branches, tags, HEAD, the reflog, worktrees, a
//! staging area for a few files, the stash, bisect and a remote called
//! origin. Commands are parsed from the line the player types and answer
//! with the lines real git prints, shortened.
//!
//! Nothing here is saved: a level is its setup commands plus the commands
//! the player ran, so a save replays them.

/// Output line kinds.
pub const OUT: u8 = 0;
pub const ERR: u8 = 1;
pub const GOOD: u8 = 2;
pub const HINT: u8 = 3;
pub const CMD: u8 = 4;
/// a failing test: red, but not an error
pub const FAIL: u8 = 5;

#[derive(Clone)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<usize>,
    pub msg: String,
    /// what it changes, for comparing with the goal: the setup commits it
    /// carries (one bit each), how many new changes, which it reverts, how
    /// often it was amended, and the files it touches
    pub fixed: u64,
    pub news: u32,
    pub rev: u64,
    pub revnew: u32,
    pub amends: u32,
    pub files: u32,
    /// the same change keeps its patch through cherry-picks and rebases
    pub patch: u32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Head {
    Branch(String),
    Detached(usize),
}

#[derive(Clone)]
pub struct Worktree {
    pub path: String,
    pub head: Head,
    /// where `git switch -` goes
    pub prev: Option<Head>,
}

#[derive(Clone, Default)]
pub struct Repo {
    pub commits: Vec<Commit>,
    pub branches: Vec<(String, usize)>,
    pub tags: Vec<(String, usize)>,
    /// remote-tracking branches, "origin/main"
    pub remote: Vec<(String, usize)>,
    /// local branch, and the branch on origin it tracks
    pub upstream: Vec<(String, String)>,
    pub wts: Vec<Worktree>,
    pub cur: usize,
    /// where HEAD was, newest last, with what moved it
    pub reflog: Vec<(usize, String)>,
    pub orig_head: Option<usize>,
}

#[derive(Clone)]
pub struct File {
    pub name: String,
    pub tracked: bool,
    pub staged: bool,
    pub modified: bool,
}

#[derive(Clone)]
pub struct Bisect {
    pub good: Vec<usize>,
    pub bad: Option<usize>,
    pub orig: Head,
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

#[derive(Clone)]
pub struct World {
    pub inited: bool,
    pub repo: Repo,
    pub origin: Option<Repo>,
    pub files: Vec<File>,
    pub stash: Vec<(String, Vec<(usize, bool, bool)>)>,
    pub bisect: Option<Bisect>,
    /// the commit that broke the tests, for `make test`
    pub bug: Option<usize>,
    pub todo: Option<Todo>,
    /// whether `git commit` needs staged files (the first lessons)
    pub uses_files: bool,
    pub pull_rebase: Option<bool>,
    pub auto_remote: bool,
    pub seed: u32,
    pub next_id: u32,
    pub next_patch: u32,
    pub out: Vec<(u8, String)>,
    /// setup commands run on origin (a teammate's work)
    on_origin: bool,
}

pub const REPO: &str = "app";

type R<T> = Result<T, String>;

fn hash(a: u32, b: u32) -> u32 {
    let mut h = a ^ b.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

const MESSAGES: [&str; 8] = [
    "Add a feature",
    "Fix a bug",
    "Tidy up",
    "Write tests",
    "Improve docs",
    "Speed things up",
    "Polish",
    "Refactor",
];

/// Splits a line into commands (on && and ;) and words, with quotes.
pub fn split(line: &str) -> Vec<Vec<String>> {
    let mut cmds = vec![];
    let mut words: Vec<String> = vec![];
    let mut word = String::new();
    let mut has = false;
    let mut quote: Option<char> = None;
    let cs: Vec<char> = line.chars().collect();
    let mut i = 0;
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
            if has {
                words.push(std::mem::take(&mut word));
                has = false;
            }
        } else if c == ';' || (c == '&' && cs.get(i + 1) == Some(&'&')) {
            if has {
                words.push(std::mem::take(&mut word));
                has = false;
            }
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
    if has {
        words.push(word);
    }
    if !words.is_empty() {
        cmds.push(words);
    }
    cmds
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with('-')
        && !n.ends_with('/')
        && !n.eq_ignore_ascii_case("head")
        && !n.contains("..")
        && !n.contains("@{")
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_/.".contains(c))
}

/// "../hotfix/" -> "hotfix"
fn base_path(p: &str) -> String {
    p.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(p)
        .to_string()
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

impl Repo {
    pub fn wt(&self) -> &Worktree {
        &self.wts[self.cur]
    }
    pub fn head(&self) -> &Head {
        &self.wts[self.cur].head
    }
    pub fn branch(&self, name: &str) -> Option<usize> {
        self.branches.iter().find(|b| b.0 == name).map(|b| b.1)
    }
    pub fn tag(&self, name: &str) -> Option<usize> {
        self.tags.iter().find(|b| b.0 == name).map(|b| b.1)
    }
    pub fn head_of(&self, h: &Head) -> Option<usize> {
        match h {
            Head::Branch(b) => self.branch(b),
            Head::Detached(c) => Some(*c),
        }
    }
    pub fn head_commit(&self) -> Option<usize> {
        self.head_of(self.head())
    }
    pub fn cur_branch(&self) -> Option<&str> {
        match self.head() {
            Head::Branch(b) => Some(b),
            _ => None,
        }
    }
    pub fn find_id(&self, id: &str) -> Option<usize> {
        self.commits.iter().position(|c| c.id == id)
    }
    pub fn set_branch(&mut self, name: &str, c: usize) {
        if let Some(b) = self.branches.iter_mut().find(|b| b.0 == name) {
            b.1 = c;
        } else {
            self.branches.push((name.to_string(), c));
        }
    }
    pub fn set_ref(list: &mut Vec<(String, usize)>, name: &str, c: usize) {
        if let Some(b) = list.iter_mut().find(|b| b.0 == name) {
            b.1 = c;
        } else {
            list.push((name.to_string(), c));
        }
    }
    pub fn upstream_of(&self, branch: &str) -> Option<String> {
        self.upstream
            .iter()
            .find(|u| u.0 == branch)
            .map(|u| u.1.clone())
    }
    pub fn set_upstream(&mut self, branch: &str, up: &str) {
        self.upstream.retain(|u| u.0 != branch);
        self.upstream.push((branch.to_string(), up.to_string()));
    }
    /// Which worktree has this branch checked out.
    pub fn checked_out(&self, branch: &str) -> Option<usize> {
        self.wts
            .iter()
            .position(|w| w.head == Head::Branch(branch.to_string()))
    }
    /// Every ancestor of c, c included.
    pub fn ancestors(&self, c: usize) -> Vec<bool> {
        let mut seen = vec![false; self.commits.len()];
        let mut stack = vec![c];
        while let Some(x) = stack.pop() {
            if seen[x] {
                continue;
            }
            seen[x] = true;
            stack.extend(self.commits[x].parents.iter().copied());
        }
        seen
    }
    pub fn is_ancestor(&self, a: usize, b: usize) -> bool {
        self.ancestors(b)[a]
    }
    /// Commits reachable from `to` but not from `from`, oldest first.
    pub fn range(&self, from: Option<usize>, to: usize) -> Vec<usize> {
        let not = from
            .map(|f| self.ancestors(f))
            .unwrap_or_else(|| vec![false; self.commits.len()]);
        let yes = self.ancestors(to);
        (0..self.commits.len())
            .filter(|&i| yes[i] && !not[i])
            .collect()
    }
    /// Moves HEAD's branch (or a detached HEAD) to c.
    pub fn move_head(&mut self, c: usize, why: String) {
        match self.head().clone() {
            Head::Branch(b) => self.set_branch(&b, c),
            Head::Detached(_) => self.wts[self.cur].head = Head::Detached(c),
        }
        if self.cur == 0 {
            self.reflog.push((c, why));
        }
    }
    /// Points HEAD somewhere else (a switch).
    pub fn set_head(&mut self, h: Head, why: String) {
        let old = self.head().clone();
        if old != h {
            self.wts[self.cur].prev = Some(old);
        }
        self.wts[self.cur].head = h;
        if let (Some(c), 0) = (self.head_commit(), self.cur) {
            self.reflog.push((c, why));
        }
    }
    pub fn subject(&self, c: usize) -> String {
        format!("{} {}", self.commits[c].id, self.commits[c].msg)
    }
    /// The names shown next to a commit in `git log`.
    pub fn decorations(&self, c: usize) -> String {
        let mut d = vec![];
        let head = self.head();
        if *head == Head::Detached(c) {
            d.push("HEAD".to_string());
        }
        for (name, at) in &self.branches {
            if *at == c {
                if *head == Head::Branch(name.clone()) {
                    d.insert(0, format!("HEAD -> {name}"));
                } else {
                    d.push(name.clone());
                }
            }
        }
        for (name, at) in &self.remote {
            if *at == c {
                d.push(name.clone());
            }
        }
        for (name, at) in &self.tags {
            if *at == c {
                d.push(format!("tag: {name}"));
            }
        }
        if d.is_empty() {
            String::new()
        } else {
            format!(" ({})", d.join(", "))
        }
    }
    /// A fingerprint of each commit's change and history, the same for two
    /// commits that hold the same work on the same history whatever their ids.
    pub fn sigs(&self) -> Vec<u64> {
        let mut s: Vec<u64> = Vec::with_capacity(self.commits.len());
        for c in &self.commits {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            let mut mix = |v: u64| {
                h ^= v;
                h = h.wrapping_mul(0x0100_0000_01b3);
                h ^= h >> 29;
            };
            mix(c.fixed);
            mix(c.news as u64);
            mix(c.rev);
            mix(c.revnew as u64);
            mix(c.amends as u64);
            mix(c.files as u64);
            mix(c.parents.len() as u64);
            for &p in &c.parents {
                mix(s[p]);
            }
            s.push(h);
        }
        s
    }
    /// Copies commit `i` of `src` (and the history it needs) in, by id.
    pub fn import(&mut self, src: &Repo, i: usize) -> usize {
        if let Some(j) = self.find_id(&src.commits[i].id) {
            return j;
        }
        let mut c = src.commits[i].clone();
        c.parents = src.commits[i]
            .parents
            .iter()
            .map(|&p| self.import(src, p))
            .collect();
        self.commits.push(c);
        self.commits.len() - 1
    }
}

/// What the goal is checked against.
#[derive(PartialEq, Debug)]
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
    files: Vec<(bool, bool, bool)>,
    stash: usize,
}

fn refs_sig(list: &[(String, usize)], sig: &[u64]) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = list.iter().map(|(n, c)| (n.clone(), sig[*c])).collect();
    v.sort();
    v
}

impl World {
    pub fn new(seed: u32, files: &[&str]) -> World {
        World {
            inited: false,
            repo: Repo::default(),
            origin: None,
            files: files
                .iter()
                .map(|f| File {
                    name: f.to_string(),
                    tracked: false,
                    staged: false,
                    modified: true,
                })
                .collect(),
            stash: vec![],
            bisect: None,
            bug: None,
            todo: None,
            uses_files: !files.is_empty(),
            pull_rebase: None,
            auto_remote: false,
            seed,
            next_id: 0,
            next_patch: 0,
            out: vec![],
            on_origin: false,
        }
    }

    pub fn snap(&self) -> Snap {
        let r = &self.repo;
        let sig = r.sigs();
        let head = |h: &Head| match h {
            Head::Branch(b) => format!("b:{b}"),
            Head::Detached(c) => format!("c:{}", sig[*c]),
        };
        let mut heads: Vec<(String, String)> = r
            .wts
            .iter()
            .map(|w| (w.path.clone(), head(&w.head)))
            .collect();
        heads.sort();
        let mut upstream = r.upstream.clone();
        upstream.retain(|u| r.branch(&u.0).is_some());
        upstream.sort();
        let (origin, origin_tags) = match &self.origin {
            Some(o) => {
                let os = o.sigs();
                (refs_sig(&o.branches, &os), refs_sig(&o.tags, &os))
            }
            None => (vec![], vec![]),
        };
        Snap {
            inited: self.inited,
            branches: refs_sig(&r.branches, &sig),
            tags: refs_sig(&r.tags, &sig),
            remote: refs_sig(&r.remote, &sig),
            upstream,
            heads,
            cur: r.wts.get(r.cur).map(|w| w.path.clone()).unwrap_or_default(),
            origin,
            origin_tags,
            files: self
                .files
                .iter()
                .map(|f| (f.tracked, f.staged, f.modified))
                .collect(),
            stash: self.stash.len(),
        }
    }

    fn say(&mut self, kind: u8, s: impl Into<String>) {
        let s = s.into();
        for line in s.split('\n') {
            self.out.push((kind, line.to_string()));
        }
    }

    fn r(&mut self) -> &mut Repo {
        if self.on_origin {
            self.origin.as_mut().unwrap()
        } else {
            &mut self.repo
        }
    }
    fn rr(&self) -> &Repo {
        if self.on_origin {
            self.origin.as_ref().unwrap()
        } else {
            &self.repo
        }
    }

    fn new_id(&mut self) -> String {
        loop {
            self.next_id += 1;
            let id = format!("{:04x}", hash(self.seed, self.next_id) & 0xffff);
            let taken = |r: &Repo| r.find_id(&id).is_some() || r.branch(&id).is_some();
            if !taken(&self.repo)
                && !self.origin.as_ref().is_some_and(taken)
                && !id.starts_with(|c: char| c.is_ascii_digit())
            {
                return id;
            }
        }
    }
    fn new_patch(&mut self) -> u32 {
        self.next_patch += 1;
        self.next_patch
    }

    /// Makes a commit on `parents`; returns its index.
    fn make(&mut self, parents: Vec<usize>, msg: String, base: Option<Commit>) -> usize {
        let id = self.new_id();
        let patch = self.new_patch();
        let c = match base {
            Some(mut b) => {
                b.id = id;
                b.parents = parents;
                b.msg = msg;
                b
            }
            None => Commit {
                id,
                parents,
                msg,
                fixed: 0,
                news: 1,
                rev: 0,
                revnew: 0,
                amends: 0,
                files: 0,
                patch,
            },
        };
        let r = self.r();
        r.commits.push(c);
        r.commits.len() - 1
    }

    /// After a level's setup: each commit made so far is its own change.
    pub fn freeze(&mut self) {
        for r in std::iter::once(&mut self.repo).chain(self.origin.as_mut()) {
            for (i, c) in r.commits.iter_mut().enumerate() {
                if c.news > 0 || c.fixed == 0 {
                    c.fixed = 1u64 << (i % 64);
                    c.news = 0;
                }
            }
        }
        // origin and the clone share commits: give them the same bits by id
        if let Some(o) = &mut self.origin {
            for c in o.commits.iter_mut() {
                if let Some(j) = self.repo.find_id(&c.id) {
                    c.fixed = self.repo.commits[j].fixed;
                }
            }
        }
        self.out.clear();
        self.on_origin = false;
    }

    // ---------- revisions ----------

    /// A commit from a name: HEAD, @, a branch, tag, origin/x, an id, then
    /// any of ~n ^n @{n}.
    pub fn resolve(&self, spec: &str) -> R<usize> {
        self.resolve_in(self.rr(), spec)
    }
    fn resolve_in(&self, r: &Repo, spec: &str) -> R<usize> {
        let bad = || format!("fatal: ambiguous argument '{spec}': unknown revision");
        let cut = spec.find(['~', '^']).unwrap_or(spec.len());
        let (base, mut rest) = spec.split_at(cut);
        let mut c = if let Some(at) = base.find("@{") {
            let (name, n) = base.split_at(at);
            let n = n.trim_start_matches("@{").trim_end_matches('}');
            if n == "u" || n == "upstream" {
                let b = if name.is_empty() || name.eq_ignore_ascii_case("head") {
                    r.cur_branch()
                        .ok_or_else(|| "fatal: HEAD does not point to a branch".to_string())?
                        .to_string()
                } else {
                    name.to_string()
                };
                let up = r
                    .upstream_of(&b)
                    .ok_or_else(|| format!("fatal: no upstream configured for branch '{b}'"))?;
                r.remote
                    .iter()
                    .find(|x| x.0 == format!("origin/{up}"))
                    .map(|x| x.1)
                    .ok_or_else(bad)?
            } else if let Some(k) = n.strip_prefix('-') {
                let k: usize = k.parse().map_err(|_| bad())?;
                // only the last place is kept
                let prev = if k == 1 { r.wt().prev.clone() } else { None };
                r.head_of(&prev.ok_or_else(|| "fatal: no previous branch".to_string())?)
                    .ok_or_else(bad)?
            } else {
                let k: usize = n.parse().map_err(|_| bad())?;
                if !(name.is_empty() || name.eq_ignore_ascii_case("head")) {
                    return Err("fatal: only HEAD has a reflog here".into());
                }
                let log = &r.reflog;
                if k >= log.len() {
                    return Err(format!(
                        "fatal: log for 'HEAD' only has {} entries",
                        log.len()
                    ));
                }
                log[log.len() - 1 - k].0
            }
        } else if base.eq_ignore_ascii_case("head") || base == "@" {
            r.head_commit().ok_or_else(|| {
                "fatal: ambiguous argument 'HEAD': there are no commits yet".to_string()
            })?
        } else if base == "ORIG_HEAD" {
            r.orig_head.ok_or_else(bad)?
        } else if let Some(c) = r.branch(base) {
            c
        } else if let Some(c) = r.tag(base) {
            c
        } else if let Some(c) = r
            .remote
            .iter()
            .find(|x| x.0 == base || x.0 == format!("origin/{base}"))
            .map(|x| x.1)
        {
            c
        } else if let Some(c) = r.find_id(base) {
            c
        } else if base == "bisect/bad" || base == "refs/bisect/bad" {
            self.bisect
                .as_ref()
                .and_then(|b| b.found.or(b.bad))
                .ok_or_else(bad)?
        } else if let Some(name) = base
            .strip_prefix("refs/heads/")
            .or(base.strip_prefix("heads/"))
        {
            r.branch(name).ok_or_else(bad)?
        } else if let Some(name) = base
            .strip_prefix("refs/tags/")
            .or(base.strip_prefix("tags/"))
        {
            r.tag(name).ok_or_else(bad)?
        } else {
            return Err(bad());
        };
        while !rest.is_empty() {
            let op = rest.as_bytes()[0];
            rest = &rest[1..];
            let digits = rest
                .find(|ch: char| !ch.is_ascii_digit())
                .unwrap_or(rest.len());
            let n: usize = if digits == 0 {
                1
            } else {
                rest[..digits].parse().map_err(|_| bad())?
            };
            rest = &rest[digits..];
            if op == b'~' {
                for _ in 0..n {
                    c = *r.commits[c].parents.first().ok_or_else(bad)?;
                }
            } else if n > 0 {
                c = *r.commits[c].parents.get(n - 1).ok_or_else(bad)?;
            }
        }
        Ok(c)
    }

    // ---------- running a line ----------

    /// Runs a typed line; true when it changed anything (and so is kept
    /// for the save and undo).
    pub fn run(&mut self, line: &str) -> bool {
        let mut changed = false;
        for words in split(line) {
            let before = self.fingerprint();
            let ok = match self.exec(&words) {
                Ok(()) => true,
                Err(e) => {
                    if !e.is_empty() {
                        self.say(ERR, e);
                    }
                    false
                }
            };
            changed |= self.fingerprint() != before;
            if !ok {
                break;
            }
        }
        changed
    }

    /// Changes whenever the state does (not the output).
    fn fingerprint(&self) -> u64 {
        let mut h: u64 = 1469598103934665603;
        let mut add = |s: &str| {
            for b in s.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(1099511628211);
            }
            h ^= 0xff;
        };
        for r in std::iter::once(&self.repo).chain(self.origin.as_ref()) {
            add(&r.commits.len().to_string());
            for (n, c) in r.branches.iter().chain(&r.tags).chain(&r.remote) {
                add(n);
                add(&c.to_string());
            }
            for (a, b) in &r.upstream {
                add(a);
                add(b);
            }
            for w in &r.wts {
                add(&w.path);
                add(&format!("{:?}", w.head));
            }
            add(&r.cur.to_string());
            add(&r.reflog.len().to_string());
        }
        for f in &self.files {
            add(&format!("{}{}{}", f.tracked, f.staged, f.modified));
        }
        add(&self.stash.len().to_string());
        add(&format!(
            "{}{:?}{:?}{}",
            self.inited, self.pull_rebase, self.bug, self.auto_remote
        ));
        if let Some(b) = &self.bisect {
            add(&format!("{:?}{:?}{:?}", b.good, b.bad, b.found));
        } else {
            add("-");
        }
        h
    }

    fn exec(&mut self, w: &[String]) -> R<()> {
        let args: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
        match args[0] {
            "git" => {}
            "@origin" => {
                self.on_origin = true;
                let r = self.exec(&w[1..]);
                self.on_origin = false;
                return r;
            }
            "#clone" => return self.clone_to_origin(),
            "#bug" => {
                let msg = args[1..].join(" ");
                self.bug = self.repo.commits.iter().position(|c| c.msg == msg);
                return Ok(());
            }
            "edit" | "touch" => return self.edit(&args[1..]),
            "cd" => return self.cd(args.get(1).copied().unwrap_or("")),
            "make" | "test" | "./test" | "npm" => return self.test(),
            "ls" => {
                let names: Vec<String> = self.files.iter().map(|f| f.name.clone()).collect();
                self.say(
                    OUT,
                    if names.is_empty() {
                        "src".to_string()
                    } else {
                        names.join("  ")
                    },
                );
                return Ok(());
            }
            "pwd" => {
                let p = self.repo.wt().path.clone();
                self.say(
                    OUT,
                    format!("/home/you/{}", if p.is_empty() { REPO } else { &p }),
                );
                return Ok(());
            }
            other => return Err(format!("{other}: command not found (try git, or hint)")),
        }
        let Some(&sub) = args.get(1) else {
            self.say(OUT, "usage: git <command> [<args>]  (try git status)");
            return Ok(());
        };
        let a = &args[2..];
        if sub == "init" {
            return self.init(a);
        }
        if sub == "help" || sub == "--help" || sub == "version" || sub == "--version" {
            self.say(
                OUT,
                if sub.contains("version") {
                    "git version 2.51.0"
                } else {
                    "Type hint for a nudge, or solution for the answer."
                },
            );
            return Ok(());
        }
        if !self.inited && !self.on_origin {
            return Err(
                "fatal: not a git repository (or any of the parent directories): .git".into(),
            );
        }
        match sub {
            "status" => self.status(),
            "add" => self.add(a),
            "commit" => self.commit(a),
            "restore" => self.restore(a),
            "rm" => self.rm(a),
            "diff" => self.diff(a),
            "log" => self.log(a),
            "show" => self.show(a),
            "branch" => self.branch(a),
            "switch" => self.switch(a),
            "checkout" => self.checkout(a),
            "merge" => self.merge(a),
            "rebase" => self.rebase(a),
            "cherry-pick" => self.cherry_pick(a),
            "revert" => self.revert(a),
            "reset" => self.reset(a),
            "tag" => self.tag(a),
            "describe" => self.describe(a),
            "reflog" => self.reflog(),
            "stash" => self.stash_cmd(a),
            "bisect" => self.bisect_cmd(a),
            "worktree" => self.worktree(a),
            "fetch" => self.fetch(a),
            "pull" => self.pull(a),
            "push" => self.push(a),
            "remote" => self.remote(a),
            "config" => self.config(a),
            "clone" => Err(
                "fatal: destination path 'app' already exists and is not an empty directory."
                    .into(),
            ),
            _ => Err(format!(
                "git: '{sub}' is not a git command here. Try git status, or hint."
            )),
        }
    }

    fn init(&mut self, a: &[&str]) -> R<()> {
        if self.inited {
            self.say(
                OUT,
                "Reinitialized existing Git repository in /home/you/app/.git/",
            );
            return Ok(());
        }
        let mut name = "main".to_string();
        let mut i = 0;
        while i < a.len() {
            match a[i] {
                "-b" | "--initial-branch" => {
                    name = a
                        .get(i + 1)
                        .ok_or("error: switch `b' requires a value")?
                        .to_string();
                    i += 1;
                }
                s if s.starts_with("--initial-branch=") => name = s[17..].to_string(),
                _ => {}
            }
            i += 1;
        }
        self.inited = true;
        self.repo.wts = vec![Worktree {
            path: REPO.into(),
            head: Head::Branch(name),
            prev: None,
        }];
        self.say(
            OUT,
            "Initialized empty Git repository in /home/you/app/.git/",
        );
        Ok(())
    }

    // ---------- files ----------

    fn file(&self, name: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|f| f.name == name || format!("./{}", f.name) == name)
    }
    /// Files named by pathspecs ("." for all), or an error naming the first
    /// unknown one.
    fn paths(&self, specs: &[&str]) -> R<Vec<usize>> {
        let mut v = vec![];
        for &s in specs {
            if s == "." || s == "*" || s == "-A" || s == "--all" || s == ":/" {
                v.extend(0..self.files.len());
            } else if let Some(i) = self.file(s) {
                v.push(i);
            } else {
                return Err(format!(
                    "error: pathspec '{s}' did not match any file(s) known to git"
                ));
            }
        }
        Ok(v)
    }

    fn edit(&mut self, a: &[&str]) -> R<()> {
        if a.is_empty() {
            return Err("usage: edit <file>".into());
        }
        for &n in a {
            if !valid_name(n) {
                return Err(format!("edit: bad file name '{n}'"));
            }
            match self.file(n) {
                Some(i) => self.files[i].modified = true,
                None => self.files.push(File {
                    name: n.into(),
                    tracked: false,
                    staged: false,
                    modified: true,
                }),
            }
        }
        Ok(())
    }

    fn status(&mut self) -> R<()> {
        let r = &self.repo;
        let mut out = vec![];
        if r.wts.len() > 1 && r.cur > 0 {
            out.push(format!("(worktree ../{})", r.wt().path));
        }
        match r.head() {
            Head::Branch(b) => out.push(format!("On branch {b}")),
            Head::Detached(c) => out.push(format!("HEAD detached at {}", r.commits[*c].id)),
        }
        if let Some(b) = &self.bisect {
            out.push(if b.found.is_some() {
                "You are bisecting; the first bad commit is found (git bisect reset)".into()
            } else {
                "You are currently bisecting.".into()
            });
        }
        if let (Some(b), Some(c)) = (r.cur_branch(), r.head_commit()) {
            if let Some(up) = r.upstream_of(b) {
                let name = format!("origin/{up}");
                if let Some(u) = r.remote.iter().find(|x| x.0 == name).map(|x| x.1) {
                    let ahead = r.range(Some(u), c).len();
                    let behind = r.range(Some(c), u).len();
                    out.push(match (ahead, behind) {
                        (0, 0) => format!("Your branch is up to date with '{name}'."),
                        (a, 0) => format!("Your branch is ahead of '{name}' by {}.", plural(a, "commit")),
                        (0, b) => format!("Your branch is behind '{name}' by {}; git pull will fast-forward.", plural(b, "commit")),
                        (a, b) => format!("Your branch and '{name}' have diverged: {a} and {b} different commits."),
                    });
                }
            }
        }
        if r.head_commit().is_none() {
            out.push("No commits yet".into());
        }
        if self.uses_files {
            let staged: Vec<String> = self
                .files
                .iter()
                .filter(|f| f.staged)
                .map(|f| {
                    format!(
                        "    {} {}",
                        if f.tracked { "modified:" } else { "new file:" },
                        f.name
                    )
                })
                .collect();
            let mods: Vec<String> = self
                .files
                .iter()
                .filter(|f| f.modified && f.tracked)
                .map(|f| format!("    modified: {}", f.name))
                .collect();
            let untracked: Vec<String> = self
                .files
                .iter()
                .filter(|f| f.modified && !f.tracked && !f.staged)
                .map(|f| format!("    {}", f.name))
                .collect();
            if !staged.is_empty() {
                out.push("Changes to be committed:".into());
                out.extend(staged);
            }
            if !mods.is_empty() {
                out.push("Changes not staged for commit:".into());
                out.extend(mods);
            }
            if !untracked.is_empty() {
                out.push("Untracked files:".into());
                out.extend(untracked);
            }
            if !self.files.iter().any(|f| f.staged || f.modified) {
                out.push("nothing to commit, working tree clean".into());
            }
        } else {
            out.push("nothing to commit, working tree clean".into());
        }
        for l in out {
            self.say(OUT, l);
        }
        Ok(())
    }

    fn add(&mut self, a: &[&str]) -> R<()> {
        if !self.uses_files {
            return Ok(());
        }
        let specs: Vec<&str> = a.iter().copied().filter(|s| *s != "-u" || true).collect();
        if specs.is_empty() {
            return Err(
                "Nothing specified, nothing added.\nhint: Maybe you wanted to say 'git add .'?"
                    .into(),
            );
        }
        let only_tracked = specs.contains(&"-u") || specs.contains(&"--update");
        let specs: Vec<&str> = specs
            .into_iter()
            .map(|s| if s == "-u" || s == "--update" { "." } else { s })
            .collect();
        for i in self.paths(&specs).map_err(|e| {
            e.replace("error: pathspec", "fatal: pathspec")
                .replace(" file(s) known to git", " files")
        })? {
            let f = &mut self.files[i];
            if f.modified && (f.tracked || !only_tracked) {
                f.modified = false;
                f.staged = true;
            }
        }
        Ok(())
    }

    fn rm(&mut self, a: &[&str]) -> R<()> {
        let cached = a.contains(&"--cached");
        let specs: Vec<&str> = a.iter().copied().filter(|s| !s.starts_with('-')).collect();
        if !cached {
            return Err(
                "This lesson keeps its files; try git rm --cached <file> or git restore.".into(),
            );
        }
        for i in self.paths(&specs)? {
            let f = &mut self.files[i];
            f.tracked = false;
            f.staged = false;
            f.modified = true;
        }
        Ok(())
    }

    fn diff(&mut self, a: &[&str]) -> R<()> {
        let staged = a.contains(&"--staged") || a.contains(&"--cached");
        let names: Vec<String> = self
            .files
            .iter()
            .filter(|f| {
                if staged {
                    f.staged
                } else {
                    f.modified && f.tracked
                }
            })
            .map(|f| format!("diff --git a/{0} b/{0}", f.name))
            .collect();
        for n in names {
            self.say(OUT, n);
        }
        Ok(())
    }

    fn restore(&mut self, a: &[&str]) -> R<()> {
        let mut staged = false;
        let mut worktree = false;
        let mut specs = vec![];
        for &s in a {
            match s {
                "--staged" | "-S" => staged = true,
                "--worktree" | "-W" => worktree = true,
                "-SW" | "-WS" => {
                    staged = true;
                    worktree = true
                }
                s if s.starts_with("--source") || s.starts_with('-') => {}
                s => specs.push(s),
            }
        }
        if !staged {
            worktree = true;
        }
        if specs.is_empty() {
            return Err("fatal: you must specify path(s) to restore".into());
        }
        let list = self.paths(&specs)?;
        for &i in &list {
            let f = &self.files[i];
            if worktree && !staged && !f.tracked && !f.staged && list.len() == 1 {
                return Err(format!(
                    "error: pathspec '{}' did not match any file(s) known to git",
                    f.name
                ));
            }
        }
        for i in list {
            let f = &mut self.files[i];
            if staged && f.staged {
                f.staged = false;
                f.modified = true;
            }
            if worktree && f.tracked && f.modified {
                f.modified = false;
            }
        }
        Ok(())
    }

    // ---------- commits ----------

    fn commit(&mut self, a: &[&str]) -> R<()> {
        let mut msg: Option<String> = None;
        let mut all = false;
        let mut amend = false;
        let mut empty = false;
        let mut fixup: Option<&str> = None;
        let mut i = 0;
        while i < a.len() {
            let s = a[i];
            match s {
                "-m" | "--message" => {
                    let m = a.get(i + 1).ok_or("error: switch `m' requires a value")?;
                    msg = Some(match msg {
                        Some(old) => format!("{old} {m}"),
                        None => m.to_string(),
                    });
                    i += 1;
                }
                "-a" | "--all" => all = true,
                "-am" | "-ma" => {
                    all = true;
                    msg = Some(
                        a.get(i + 1)
                            .ok_or("error: switch `m' requires a value")?
                            .to_string(),
                    );
                    i += 1;
                }
                "--amend" => amend = true,
                "--allow-empty" => empty = true,
                "--no-edit" | "-q" | "--quiet" | "-v" => {}
                "--fixup" => {
                    fixup = Some(
                        a.get(i + 1)
                            .ok_or("error: option `fixup' requires a value")?,
                    );
                    i += 1;
                }
                s if s.starts_with("--fixup=") => fixup = Some(&s[8..]),
                s if s.starts_with("--message=") => msg = Some(s[10..].to_string()),
                s if s.starts_with("-m") && s.len() > 2 => msg = Some(s[2..].to_string()),
                s => return Err(format!(
                    "error: unknown option or file '{s}' (put a message in quotes: -m \"Fix it\")"
                )),
            }
            i += 1;
        }
        if self.uses_files && all {
            for f in &mut self.files {
                if f.tracked && f.modified {
                    f.modified = false;
                    f.staged = true;
                }
            }
        }
        let staged: u32 = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.staged)
            .fold(0, |m, (i, _)| m | 1 << i);
        if self.uses_files && staged == 0 && !amend && !empty {
            let any = self.files.iter().any(|f| f.modified);
            return Err(if any {
                "no changes added to commit (use \"git add\" and/or \"git commit -a\")".into()
            } else {
                "nothing to commit, working tree clean".into()
            });
        }
        let head = self.rr().head_commit();
        let c = if amend {
            let h = head.ok_or("fatal: You have nothing to amend.")?;
            let mut base = self.rr().commits[h].clone();
            base.amends += 1;
            base.files |= staged;
            let parents = base.parents.clone();
            let m = msg.unwrap_or_else(|| base.msg.clone());
            let c = self.make(parents, m, Some(base));
            let p = self.new_patch();
            self.r().commits[c].patch = p;
            self.r().orig_head = Some(h);
            self.r().move_head(c, "commit (amend)".into());
            c
        } else {
            let m = match (fixup, msg) {
                (Some(t), _) => {
                    let t = self.resolve(t)?;
                    let subj = self.rr().commits[t]
                        .msg
                        .trim_start_matches("fixup! ")
                        .to_string();
                    format!("fixup! {subj}")
                }
                (None, Some(m)) => m,
                (None, None) => MESSAGES[(self.next_id as usize) % MESSAGES.len()].to_string(),
            };
            let c = self.make(head.into_iter().collect(), m, None);
            self.r().commits[c].files = staged;
            let why = format!("commit: {}", self.rr().commits[c].msg);
            self.r().move_head(c, why);
            c
        };
        for f in &mut self.files {
            if f.staged {
                f.staged = false;
                f.tracked = true;
            }
        }
        let r = self.rr();
        let where_ = match r.head() {
            Head::Branch(b) => b.clone(),
            Head::Detached(_) => "detached HEAD".into(),
        };
        let root = if r.commits[c].parents.is_empty() {
            " (root-commit)"
        } else {
            ""
        };
        let line = format!("[{where_}{root} {}] {}", r.commits[c].id, r.commits[c].msg);
        let n = staged.count_ones() as usize;
        if !self.on_origin {
            self.say(OUT, line);
            if self.uses_files && n > 0 {
                self.say(OUT, format!(" {} changed", plural(n, "file")));
            }
        }
        Ok(())
    }

    fn log(&mut self, a: &[&str]) -> R<()> {
        let all = a.contains(&"--all");
        let mut n = 10;
        let mut start = None;
        let mut i = 0;
        while i < a.len() {
            let s = a[i];
            if s == "-n" {
                n = a.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(10);
                i += 1;
            } else if let Some(k) = s.strip_prefix('-').and_then(|k| k.parse::<usize>().ok()) {
                n = k;
            } else if !s.starts_with('-') {
                start = Some(self.resolve(s)?);
            }
            i += 1;
        }
        let r = &self.repo;
        let mut seen = vec![false; r.commits.len()];
        let mut tips: Vec<usize> = vec![];
        if all {
            tips.extend(
                r.branches
                    .iter()
                    .chain(&r.remote)
                    .chain(&r.tags)
                    .map(|b| b.1),
            );
            tips.extend(r.wts.iter().filter_map(|w| r.head_of(&w.head)));
        }
        match start.or(r.head_commit()) {
            Some(c) => tips.push(c),
            None if !all => {
                let b = r.cur_branch().unwrap_or("main").to_string();
                return Err(format!(
                    "fatal: your current branch '{b}' does not have any commits yet"
                ));
            }
            None => {}
        }
        for t in tips {
            let anc = r.ancestors(t);
            for (k, v) in anc.into_iter().enumerate() {
                seen[k] |= v;
            }
        }
        let lines: Vec<String> = (0..r.commits.len())
            .rev()
            .filter(|&k| seen[k])
            .take(n)
            .map(|k| {
                format!(
                    "{}{} {}",
                    r.commits[k].id,
                    r.decorations(k),
                    r.commits[k].msg
                )
            })
            .collect();
        for l in lines {
            self.say(OUT, l);
        }
        Ok(())
    }

    fn show(&mut self, a: &[&str]) -> R<()> {
        let c = self.resolve(
            a.iter()
                .find(|s| !s.starts_with('-'))
                .copied()
                .unwrap_or("HEAD"),
        )?;
        let r = &self.repo;
        let k = &r.commits[c];
        let mut lines = vec![format!("commit {}{}", k.id, r.decorations(c))];
        if k.parents.len() > 1 {
            lines.push(format!(
                "Merge: {}",
                k.parents
                    .iter()
                    .map(|&p| r.commits[p].id.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        lines.push(format!("    {}", k.msg));
        for (i, f) in self.files.iter().enumerate() {
            if k.files & 1 << i != 0 {
                lines.push(format!(" {} changed", f.name));
            }
        }
        for l in lines {
            self.say(OUT, l);
        }
        Ok(())
    }

    // ---------- branches ----------

    fn branch(&mut self, a: &[&str]) -> R<()> {
        let flags: Vec<&str> = a.iter().copied().filter(|s| s.starts_with('-')).collect();
        let pos: Vec<&str> = a.iter().copied().filter(|s| !s.starts_with('-')).collect();
        let has = |f: &[&str]| flags.iter().any(|x| f.contains(x));
        if has(&["--show-current"]) {
            let b = self.repo.cur_branch().unwrap_or("").to_string();
            self.say(OUT, b);
            return Ok(());
        }
        if let Some(up) = flags
            .iter()
            .find_map(|f| f.strip_prefix("--set-upstream-to="))
            .or(if has(&["-u", "--set-upstream-to"]) {
                pos.first().copied()
            } else {
                None
            })
        {
            let b = match (has(&["-u", "--set-upstream-to"]), pos.len()) {
                (true, 2) => pos[1].to_string(),
                (false, 1) => pos[0].to_string(),
                _ => self.repo.cur_branch().ok_or("fatal: could not set upstream of HEAD when it does not point to any branch")?.to_string(),
            };
            let up = up.trim_start_matches("origin/");
            if !self
                .repo
                .remote
                .iter()
                .any(|x| x.0 == format!("origin/{up}"))
            {
                return Err(format!(
                    "fatal: the requested upstream branch 'origin/{up}' does not exist"
                ));
            }
            self.repo.set_upstream(&b, up);
            self.say(OUT, format!("branch '{b}' set up to track 'origin/{up}'."));
            return Ok(());
        }
        if has(&["-d", "-D", "--delete"]) {
            if pos.is_empty() {
                return Err("fatal: branch name required".into());
            }
            for name in pos {
                let c = self
                    .rr()
                    .branch(name)
                    .ok_or_else(|| format!("error: branch '{name}' not found"))?;
                if let Some(w) = self.rr().checked_out(name) {
                    let p = self.rr().wts[w].path.clone();
                    return Err(format!(
                        "error: cannot delete branch '{name}' used by worktree at '{}'",
                        if w == 0 {
                            format!("/home/you/{p}")
                        } else {
                            format!("../{p}")
                        }
                    ));
                }
                if !has(&["-D"]) {
                    let r = self.rr();
                    let into = r.upstream_of(name).and_then(|u| {
                        r.remote
                            .iter()
                            .find(|x| x.0 == format!("origin/{u}"))
                            .map(|x| x.1)
                    });
                    let merged = r.head_commit().is_some_and(|h| r.is_ancestor(c, h))
                        || into.is_some_and(|u| r.is_ancestor(c, u));
                    if !merged {
                        return Err(format!("error: the branch '{name}' is not fully merged\nhint: If you are sure you want to delete it, run 'git branch -D {name}'"));
                    }
                }
                let id = self.rr().commits[c].id.clone();
                self.r().branches.retain(|b| b.0 != name);
                self.r().upstream.retain(|u| u.0 != name);
                if !self.on_origin {
                    self.say(OUT, format!("Deleted branch {name} (was {id})."));
                }
            }
            return Ok(());
        }
        if has(&["-m", "-M", "--move"]) {
            let (old, new) = match pos.len() {
                1 => (
                    self.repo
                        .cur_branch()
                        .ok_or("fatal: no branch to rename")?
                        .to_string(),
                    pos[0].to_string(),
                ),
                2 => (pos[0].to_string(), pos[1].to_string()),
                _ => return Err("fatal: branch name required".into()),
            };
            self.repo
                .branch(&old)
                .ok_or_else(|| format!("error: refname refs/heads/{old} not found"))?;
            if !valid_name(&new) {
                return Err(format!("fatal: '{new}' is not a valid branch name"));
            }
            if self.repo.branch(&new).is_some() && !has(&["-M"]) {
                return Err(format!("fatal: a branch named '{new}' already exists"));
            }
            let r = &mut self.repo;
            r.branches.retain(|b| b.0 != new);
            for b in r.branches.iter_mut().filter(|b| b.0 == old) {
                b.0 = new.clone();
            }
            for u in r.upstream.iter_mut().filter(|u| u.0 == old) {
                u.0 = new.clone();
            }
            for w in r.wts.iter_mut() {
                if w.head == Head::Branch(old.clone()) {
                    w.head = Head::Branch(new.clone());
                }
            }
            return Ok(());
        }
        let list_remote = has(&["-r", "--remotes"]);
        let list_all = has(&["-a", "--all"]);
        let verbose = has(&["-v", "-vv", "--verbose"]);
        if pos.is_empty() || has(&["--list"]) {
            let r = &self.repo;
            let mut lines = vec![];
            if !list_remote {
                for (name, c) in &r.branches {
                    let star = if *r.head() == Head::Branch(name.clone()) {
                        "* "
                    } else if r.checked_out(name).is_some() {
                        "+ "
                    } else {
                        "  "
                    };
                    let mut l = format!("{star}{name}");
                    if verbose {
                        l += &format!(" {}", r.commits[*c].id);
                        if let Some(u) = r.upstream_of(name) {
                            l += &format!(" [origin/{u}]");
                        }
                        l += &format!(" {}", r.commits[*c].msg);
                    }
                    lines.push(l);
                }
                if let Head::Detached(c) = r.head() {
                    lines.insert(0, format!("* (HEAD detached at {})", r.commits[*c].id));
                }
            }
            if list_remote || list_all {
                for (name, _) in &r.remote {
                    lines.push(format!(
                        "  {}{name}",
                        if list_all { "remotes/" } else { "" }
                    ));
                }
            }
            for l in lines {
                self.say(OUT, l);
            }
            return Ok(());
        }
        let force = has(&["-f", "--force"]);
        let name = pos[0];
        if !valid_name(name) {
            return Err(format!("fatal: '{name}' is not a valid branch name"));
        }
        let at = match pos.get(1) {
            Some(s) => self.resolve(s)?,
            None => self
                .rr()
                .head_commit()
                .ok_or("fatal: not a valid object name: 'main' (make a commit first)")?,
        };
        if self.rr().branch(name).is_some() {
            if !force {
                return Err(format!("fatal: a branch named '{name}' already exists"));
            }
            if let Some(w) = self.rr().checked_out(name) {
                if w == self.rr().cur {
                    return Err(format!(
                        "fatal: cannot force update the current branch '{name}' (use git reset)"
                    ));
                }
                return Err(format!(
                    "fatal: cannot force update the branch '{name}' used by worktree at '../{}'",
                    self.rr().wts[w].path
                ));
            }
        }
        self.r().set_branch(name, at);
        if let Some(up) = pos.get(1).and_then(|s| s.strip_prefix("origin/")) {
            if self
                .repo
                .remote
                .iter()
                .any(|x| x.0 == format!("origin/{up}"))
                && !force
            {
                self.repo.set_upstream(name, up);
                self.say(
                    OUT,
                    format!("branch '{name}' set up to track 'origin/{up}'."),
                );
            }
        }
        Ok(())
    }

    /// Error when uncommitted edits would be overwritten by moving to `to`.
    fn would_overwrite(&self, to: usize) -> R<()> {
        if !self.uses_files {
            return Ok(());
        }
        let r = &self.repo;
        let Some(from) = r.head_commit() else {
            return Ok(());
        };
        let mut changed = 0u32;
        for c in r
            .range(Some(from), to)
            .into_iter()
            .chain(r.range(Some(to), from))
        {
            changed |= r.commits[c].files;
        }
        let hit: Vec<String> = self
            .files
            .iter()
            .enumerate()
            .filter(|(i, f)| (f.staged || f.modified) && f.tracked && changed & 1 << i != 0)
            .map(|(_, f)| format!("\t{}", f.name))
            .collect();
        if hit.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "error: Your local changes to the following files would be overwritten by checkout:\n{}\nPlease commit your changes or stash them before you switch branches.",
                hit.join("\n")
            ))
        }
    }

    /// Moves HEAD onto a branch, after the worktree and file checks.
    fn go_branch(&mut self, name: &str, why: &str) -> R<()> {
        if *self.rr().head() == Head::Branch(name.to_string()) {
            self.say(OUT, format!("Already on '{name}'"));
            return Ok(());
        }
        if let Some(w) = self.rr().checked_out(name) {
            if w != self.rr().cur {
                let p = self.rr().wts[w].path.clone();
                return Err(format!(
                    "fatal: '{name}' is already used by worktree at '{}'",
                    if w == 0 {
                        format!("/home/you/{p}")
                    } else {
                        format!("../{p}")
                    }
                ));
            }
        }
        let c = self
            .rr()
            .branch(name)
            .ok_or_else(|| format!("fatal: invalid reference: {name}"))?;
        if !self.on_origin {
            self.would_overwrite(c)?;
        }
        let from = self.head_label();
        self.r().set_head(
            Head::Branch(name.into()),
            format!("{why}: moving from {from} to {name}"),
        );
        if !self.on_origin {
            self.say(OUT, format!("Switched to branch '{name}'"));
        }
        Ok(())
    }

    fn head_label(&self) -> String {
        match self.rr().head() {
            Head::Branch(b) => b.clone(),
            Head::Detached(c) => self.rr().commits[*c].id.clone(),
        }
    }

    fn go_detached(&mut self, c: usize, why: &str) -> R<()> {
        self.would_overwrite(c)?;
        let from = self.head_label();
        let to = self.rr().commits[c].id.clone();
        self.r().set_head(
            Head::Detached(c),
            format!("{why}: moving from {from} to {to}"),
        );
        let s = self.rr().subject(c);
        self.say(OUT, format!("HEAD is now at {s}"));
        Ok(())
    }

    /// A new branch at `at` (HEAD when None), and HEAD on it.
    fn create_and_go(&mut self, name: &str, at: Option<&str>, force: bool, why: &str) -> R<()> {
        if !valid_name(name) {
            return Err(format!("fatal: '{name}' is not a valid branch name"));
        }
        if self.rr().branch(name).is_some() && !force {
            return Err(format!("fatal: a branch named '{name}' already exists"));
        }
        let c = match at {
            Some(s) => Some(self.resolve(s)?),
            None => self.rr().head_commit(),
        };
        let from = self.head_label();
        match c {
            Some(c) => {
                if !self.on_origin {
                    self.would_overwrite(c)?;
                }
                self.r().set_branch(name, c);
            }
            None => {}
        }
        self.r().set_head(
            Head::Branch(name.into()),
            format!("{why}: moving from {from} to {name}"),
        );
        if let Some(up) = at.and_then(|s| s.strip_prefix("origin/")) {
            if !self.on_origin
                && self
                    .repo
                    .remote
                    .iter()
                    .any(|x| x.0 == format!("origin/{up}"))
            {
                self.repo.set_upstream(name, up);
                self.say(
                    OUT,
                    format!("branch '{name}' set up to track 'origin/{up}'."),
                );
            }
        }
        if !self.on_origin {
            self.say(OUT, format!("Switched to a new branch '{name}'"));
        }
        Ok(())
    }

    fn previous(&self) -> R<Head> {
        self.rr()
            .wt()
            .prev
            .clone()
            .ok_or_else(|| "fatal: no previous branch to go back to".to_string())
    }

    fn switch(&mut self, a: &[&str]) -> R<()> {
        let mut create: Option<(&str, bool)> = None;
        let mut detach = false;
        let mut pos = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i] {
                "-c" | "--create" | "-C" | "--force-create" => {
                    create = Some((
                        a.get(i + 1).ok_or("error: switch `c' requires a value")?,
                        a[i] == "-C" || a[i] == "--force-create",
                    ));
                    i += 1;
                }
                "-d" | "--detach" => detach = true,
                "-t" | "--track" | "-q" | "--quiet" => {}
                "--discard-changes" | "-f" | "--force" => {}
                s if s.starts_with('-') && s != "-" => {
                    return Err(format!("error: unknown switch '{s}'"))
                }
                s => pos.push(s),
            }
            i += 1;
        }
        if let Some((name, force)) = create {
            return self.create_and_go(name, pos.first().copied(), force, "checkout");
        }
        let Some(&target) = pos.first() else {
            return Err("fatal: missing branch or commit argument".into());
        };
        if target == "-" {
            return match self.previous()? {
                Head::Branch(b) if !detach => self.go_branch(&b, "checkout"),
                h => {
                    let c = self.rr().head_of(&h).ok_or("fatal: no previous commit")?;
                    if !detach {
                        return Err("fatal: a branch is expected, got the previous commit\nhint: use git switch --detach -".into());
                    }
                    self.go_detached(c, "checkout")
                }
            };
        }
        if detach {
            let c = self.resolve(target)?;
            return self.go_detached(c, "checkout");
        }
        if self.rr().branch(target).is_some() {
            return self.go_branch(target, "checkout");
        }
        // a branch only on origin: make a local one tracking it
        let name = target.trim_start_matches("origin/");
        if !self.on_origin
            && self
                .repo
                .remote
                .iter()
                .any(|x| x.0 == format!("origin/{name}"))
            && self.repo.branch(name).is_none()
        {
            let at = format!("origin/{name}");
            self.create_and_go(name, Some(&at), false, "checkout")?;
            return Ok(());
        }
        match self.resolve(target) {
            Ok(_) => Err(format!("fatal: a branch is expected, got commit '{target}'\nhint: to look at a commit without a branch, use git switch --detach {target}")),
            Err(_) => Err(format!("fatal: invalid reference: {target}")),
        }
    }

    fn checkout(&mut self, a: &[&str]) -> R<()> {
        let mut create: Option<(&str, bool)> = None;
        let mut pos = vec![];
        let mut files = false;
        let mut detach = false;
        let mut i = 0;
        while i < a.len() {
            match a[i] {
                "-b" | "-B" => {
                    create = Some((
                        a.get(i + 1).ok_or("error: switch `b' requires a value")?,
                        a[i] == "-B",
                    ));
                    i += 1;
                }
                "--" => files = true,
                "--detach" => detach = true,
                "-t" | "--track" | "-q" | "-f" => {}
                s => pos.push(s),
            }
            i += 1;
        }
        if let Some((name, force)) = create {
            return self.create_and_go(name, pos.first().copied(), force, "checkout");
        }
        let Some(&target) = pos.first() else {
            return Err("fatal: you must name a branch, a commit or files".into());
        };
        if files
            || (self.file(target).is_some()
                && self.resolve(target).is_err()
                && self.repo.branch(target).is_none())
        {
            let mut b = vec!["--worktree"];
            b.extend(pos.iter().copied());
            return self.restore(&b);
        }
        if target == "-" {
            let h = self.previous()?;
            return match h {
                Head::Branch(b) => self.go_branch(&b, "checkout"),
                Head::Detached(c) => self.go_detached(c, "checkout"),
            };
        }
        if !detach && self.rr().branch(target).is_some() {
            return self.go_branch(target, "checkout");
        }
        let name = target.trim_start_matches("origin/");
        if !detach
            && !target.starts_with("origin/")
            && self
                .repo
                .remote
                .iter()
                .any(|x| x.0 == format!("origin/{name}"))
            && self.repo.branch(name).is_none()
        {
            let at = format!("origin/{name}");
            return self.create_and_go(name, Some(&at), false, "checkout");
        }
        let c = self.resolve(target).map_err(|_| {
            format!("error: pathspec '{target}' did not match any file(s) known to git")
        })?;
        self.go_detached(c, "checkout")?;
        self.say(
            HINT,
            "You are in 'detached HEAD' state: new commits here belong to no branch.",
        );
        Ok(())
    }

    // ---------- combining work ----------

    fn merge(&mut self, a: &[&str]) -> R<()> {
        let mut ff = 1; // 0 never, 1 when it can, 2 only
        let mut msg = None;
        let mut pos = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i] {
                "--no-ff" => ff = 0,
                "--ff-only" => ff = 2,
                "--ff" | "--no-edit" | "-q" => {}
                "--abort" | "--continue" => {
                    return Err("fatal: There is no merge in progress.".into())
                }
                "--squash" => {
                    return Err(
                        "Squash merges aren't part of these lessons; try git merge or git rebase."
                            .into(),
                    )
                }
                "-m" => {
                    msg = a.get(i + 1).map(|s| s.to_string());
                    i += 1;
                }
                s => pos.push(s),
            }
            i += 1;
        }
        if pos.is_empty() {
            let b = self
                .rr()
                .cur_branch()
                .ok_or("fatal: No remote for the current branch.")?
                .to_string();
            let up = self
                .rr()
                .upstream_of(&b)
                .ok_or("fatal: No remote for the current branch.")?;
            pos.push(Box::leak(format!("origin/{up}").into_boxed_str()));
        }
        if pos.len() > 1 {
            return Err("Merge one branch at a time here.".into());
        }
        let name = pos[0];
        let theirs = self.resolve(name)?;
        let Some(ours) = self.rr().head_commit() else {
            return Err("fatal: make a commit before merging".into());
        };
        self.merge_into(ours, theirs, name, ff, msg)
    }

    fn merge_into(
        &mut self,
        ours: usize,
        theirs: usize,
        name: &str,
        ff: u8,
        msg: Option<String>,
    ) -> R<()> {
        if self.rr().is_ancestor(theirs, ours) {
            self.say(OUT, "Already up to date.");
            return Ok(());
        }
        let (a, b) = (
            self.rr().commits[ours].id.clone(),
            self.rr().commits[theirs].id.clone(),
        );
        if ff > 0 && self.rr().is_ancestor(ours, theirs) {
            self.would_overwrite(theirs)?;
            self.r().orig_head = Some(ours);
            self.r()
                .move_head(theirs, format!("merge {name}: Fast-forward"));
            if !self.on_origin {
                self.say(OUT, format!("Updating {a}..{b}\nFast-forward"));
            }
            return Ok(());
        }
        if ff == 2 {
            return Err("hint: Diverging branches can't be fast-forwarded, you need to either merge or rebase.\nfatal: Not possible to fast-forward, aborting.".into());
        }
        let what = if name.starts_with("origin/") {
            format!("Merge remote-tracking branch '{name}'")
        } else if self.rr().branch(name).is_some() {
            format!("Merge branch '{name}'")
        } else {
            format!("Merge commit '{name}'")
        };
        let m = msg.unwrap_or(what);
        let mut base = Commit {
            id: String::new(),
            parents: vec![],
            msg: String::new(),
            fixed: 0,
            news: 0,
            rev: 0,
            revnew: 0,
            amends: 0,
            files: 0,
            patch: 0,
        };
        base.patch = self.new_patch();
        let c = self.make(vec![ours, theirs], m, Some(base));
        self.r().orig_head = Some(ours);
        self.r().move_head(
            c,
            format!("merge {name}: Merge made by the 'ort' strategy."),
        );
        if !self.on_origin {
            self.say(OUT, "Merge made by the 'ort' strategy.");
        }
        Ok(())
    }

    /// Copies commit `c` onto `onto`; returns the copy.
    fn copy(&mut self, c: usize, onto: usize) -> usize {
        let base = self.rr().commits[c].clone();
        let msg = base.msg.clone();
        let patch = base.patch;
        let n = self.make(vec![onto], msg, Some(base));
        self.r().commits[n].patch = patch;
        n
    }

    fn cherry_pick(&mut self, a: &[&str]) -> R<()> {
        let specs: Vec<&str> = a.iter().copied().filter(|s| !s.starts_with('-')).collect();
        if a.iter()
            .any(|s| *s == "--abort" || *s == "--continue" || *s == "--skip")
        {
            return Err("error: no cherry-pick or revert in progress".into());
        }
        if specs.is_empty() {
            return Err("usage: git cherry-pick <commit>...".into());
        }
        let mut list = vec![];
        for s in specs {
            if let Some((x, y)) = s.split_once("..") {
                let x = if x.is_empty() { "HEAD" } else { x };
                let y = if y.is_empty() { "HEAD" } else { y };
                let (x, y) = (self.resolve(x)?, self.resolve(y)?);
                list.extend(
                    self.rr()
                        .range(Some(x), y)
                        .into_iter()
                        .filter(|&c| self.rr().commits[c].parents.len() < 2),
                );
            } else {
                list.push(self.resolve(s)?);
            }
        }
        let Some(mut head) = self.rr().head_commit() else {
            return Err("fatal: make a commit before cherry-picking".into());
        };
        let start = head;
        for c in list {
            let r = self.rr();
            if r.commits[c].parents.len() > 1 {
                return Err(format!("error: commit {} is a merge but no -m option was given.\nfatal: cherry-pick failed", r.commits[c].id));
            }
            let anc = r.ancestors(head);
            if (0..r.commits.len()).any(|k| anc[k] && r.commits[k].patch == r.commits[c].patch) {
                return Err(format!(
                    "The previous cherry-pick is now empty: {} is already here.",
                    r.commits[c].id
                ));
            }
            head = self.copy(c, head);
            let why = format!("cherry-pick: {}", self.rr().commits[head].msg);
            self.r().move_head(head, why);
            let s = self.rr().subject(head);
            self.say(OUT, format!("[{}] {}", self.head_label(), s));
        }
        self.r().orig_head = Some(start);
        Ok(())
    }

    fn revert(&mut self, a: &[&str]) -> R<()> {
        let specs: Vec<&str> = a.iter().copied().filter(|s| !s.starts_with('-')).collect();
        if specs.is_empty() {
            return Err("usage: git revert <commit>...".into());
        }
        let Some(mut head) = self.rr().head_commit() else {
            return Err("fatal: nothing to revert".into());
        };
        for s in specs {
            let c = self.resolve(s)?;
            let k = self.rr().commits[c].clone();
            if k.parents.len() > 1 {
                return Err(format!(
                    "error: commit {} is a merge but no -m option was given.",
                    k.id
                ));
            }
            let base = Commit {
                id: String::new(),
                parents: vec![],
                msg: String::new(),
                fixed: 0,
                news: 0,
                rev: k.fixed,
                revnew: k.news,
                amends: 0,
                files: k.files,
                patch: 0,
            };
            let n = self.make(vec![head], format!("Revert \"{}\"", k.msg), Some(base));
            let p = self.new_patch();
            self.r().commits[n].patch = p;
            let why = format!("revert: {}", self.rr().commits[n].msg);
            self.r().move_head(n, why);
            head = n;
            let s = self.rr().subject(n);
            self.say(OUT, format!("[{}] {}", self.head_label(), s));
        }
        Ok(())
    }

    fn reset(&mut self, a: &[&str]) -> R<()> {
        let mut mode = "mixed";
        let mut pos = vec![];
        for &s in a {
            match s {
                "--soft" => mode = "soft",
                "--mixed" => mode = "mixed",
                "--hard" => mode = "hard",
                "--keep" | "--merge" => mode = "keep",
                "-q" | "--" => {}
                s => pos.push(s),
            }
        }
        // git reset <file>: unstage
        if !pos.is_empty()
            && pos
                .iter()
                .all(|p| self.file(p).is_some() && self.resolve(p).is_err())
        {
            let mut b = vec!["--staged"];
            b.extend(pos.iter().copied());
            return self.restore(&b);
        }
        let target = match pos.first() {
            Some(s) => self.resolve(s)?,
            None => self.rr().head_commit().ok_or("fatal: no commits yet")?,
        };
        let old = self.rr().head_commit();
        self.r().orig_head = old;
        let spec = pos.first().copied().unwrap_or("HEAD").to_string();
        self.r()
            .move_head(target, format!("reset: moving to {spec}"));
        match mode {
            "hard" => {
                for f in &mut self.files {
                    if f.tracked || f.staged {
                        f.staged = false;
                        if f.tracked {
                            f.modified = false;
                        } else {
                            f.modified = true;
                        }
                    }
                }
                let s = self.rr().subject(target);
                self.say(OUT, format!("HEAD is now at {s}"));
            }
            "mixed" => {
                for f in &mut self.files {
                    if f.staged {
                        f.staged = false;
                        f.modified = true;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn tag(&mut self, a: &[&str]) -> R<()> {
        let flags: Vec<&str> = a.iter().copied().filter(|s| s.starts_with('-')).collect();
        let mut pos = vec![];
        let mut i = 0;
        while i < a.len() {
            if a[i] == "-m" || a[i] == "--message" {
                i += 2;
                continue;
            }
            if !a[i].starts_with('-') {
                pos.push(a[i]);
            }
            i += 1;
        }
        if flags.contains(&"-d") || flags.contains(&"--delete") {
            for name in pos {
                let c = self
                    .rr()
                    .tag(name)
                    .ok_or_else(|| format!("error: tag '{name}' not found."))?;
                let id = self.rr().commits[c].id.clone();
                self.r().tags.retain(|t| t.0 != name);
                self.say(OUT, format!("Deleted tag '{name}' (was {id})"));
            }
            return Ok(());
        }
        if pos.is_empty() {
            let mut names: Vec<String> = self.rr().tags.iter().map(|t| t.0.clone()).collect();
            names.sort();
            for n in names {
                self.say(OUT, n);
            }
            return Ok(());
        }
        let name = pos[0];
        if !valid_name(name) {
            return Err(format!("fatal: '{name}' is not a valid tag name."));
        }
        let c = match pos.get(1) {
            Some(s) => self.resolve(s)?,
            None => self
                .rr()
                .head_commit()
                .ok_or("fatal: Failed to resolve 'HEAD' as a valid ref.")?,
        };
        if self.rr().tag(name).is_some() && !flags.contains(&"-f") {
            return Err(format!("fatal: tag '{name}' already exists"));
        }
        Repo::set_ref(&mut self.r().tags, name, c);
        Ok(())
    }

    fn describe(&mut self, a: &[&str]) -> R<()> {
        let c = self.resolve(
            a.iter()
                .find(|s| !s.starts_with('-'))
                .copied()
                .unwrap_or("HEAD"),
        )?;
        let r = &self.repo;
        let mut best: Option<(usize, String)> = None;
        for (name, t) in &r.tags {
            if r.is_ancestor(*t, c) {
                let n = r.range(Some(*t), c).len();
                if best.as_ref().is_none_or(|b| n < b.0) {
                    best = Some((n, name.clone()));
                }
            }
        }
        let line = match best {
            None => return Err("fatal: No names found, cannot describe anything.".into()),
            Some((0, name)) => name,
            Some((n, name)) => format!("{name}-{n}-g{}", r.commits[c].id),
        };
        self.say(OUT, line);
        Ok(())
    }

    fn reflog(&mut self) -> R<()> {
        let r = &self.repo;
        let lines: Vec<String> = r
            .reflog
            .iter()
            .rev()
            .enumerate()
            .take(10)
            .map(|(i, (c, why))| format!("{} HEAD@{{{i}}}: {why}", r.commits[*c].id))
            .collect();
        for l in lines {
            self.say(OUT, l);
        }
        Ok(())
    }

    // ---------- rebase ----------

    fn rebase(&mut self, a: &[&str]) -> R<()> {
        let mut onto: Option<&str> = None;
        let mut inter = false;
        let mut update_refs = false;
        let mut autosquash = false;
        let mut todo: Option<Vec<&str>> = None;
        let mut pos = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i] {
                "-i" | "--interactive" => inter = true,
                "--onto" => {
                    onto = Some(
                        a.get(i + 1)
                            .ok_or("error: option `onto' requires a value")?,
                    );
                    i += 1;
                }
                "--update-refs" => update_refs = true,
                "--no-update-refs" => update_refs = false,
                "--autosquash" => autosquash = true,
                "--no-autosquash" => autosquash = false,
                "--continue" | "--abort" | "--skip" => {
                    return Err("fatal: No rebase in progress?".into())
                }
                "#todo" => {
                    todo = Some(a[i + 1..].to_vec());
                    break;
                }
                s if s.starts_with("--onto=") => onto = Some(&s[7..]),
                s if s.starts_with('-') => {}
                s => pos.push(s),
            }
            i += 1;
        }
        if let Some(b) = pos.get(1) {
            let b = *b;
            if self.rr().branch(b).is_some() {
                self.go_branch(b, "rebase (start)")?;
            } else {
                let c = self.resolve(b)?;
                self.go_detached(c, "rebase (start)")?;
            }
        }
        let head = self
            .rr()
            .head_commit()
            .ok_or("fatal: no commits to rebase")?;
        let upstream = match pos.first() {
            Some(s) => self.resolve(s)?,
            None => {
                let b = self
                    .rr()
                    .cur_branch()
                    .ok_or("fatal: There is no tracking information for the current branch.")?
                    .to_string();
                let up = self
                    .rr()
                    .upstream_of(&b)
                    .ok_or("There is no tracking information for the current branch.\nPlease specify which branch you want to rebase against.")?;
                self.resolve(&format!("origin/{up}"))?
            }
        };
        let onto_c = match onto {
            Some(s) => self.resolve(s)?,
            None => upstream,
        };
        let r = self.rr();
        let onto_anc = r.ancestors(onto_c);
        let mut list: Vec<(usize, u8)> = r
            .range(Some(upstream), head)
            .into_iter()
            .filter(|&c| r.commits[c].parents.len() < 2)
            // a change already on the new base is left out
            .filter(|&c| {
                !(0..r.commits.len())
                    .any(|k| onto_anc[k] && r.commits[k].patch == r.commits[c].patch)
            })
            .map(|c| (c, b'p'))
            .collect();
        if autosquash {
            list = autosquash_order(r, list);
        }
        if inter && todo.is_none() {
            if list.is_empty() {
                return Err("Nothing to do: there are no commits to rebase.".into());
            }
            let cmd = std::iter::once("git rebase")
                .chain(a.iter().copied())
                .collect::<Vec<_>>()
                .join(" ");
            self.todo = Some(Todo {
                cmd,
                lines: list
                    .iter()
                    .enumerate()
                    .map(|(k, &(c, act))| (c, act, k))
                    .collect(),
                sel: 0,
            });
            return Ok(());
        }
        if let Some(t) = todo {
            let mut l2 = vec![];
            for e in t {
                let (act, k) = e.split_at(1);
                let k: usize = k.parse().map_err(|_| "bad to-do list")?;
                let c = list.get(k).ok_or("bad to-do list")?.0;
                l2.push((c, act.as_bytes()[0]));
            }
            list = l2;
        }
        // nothing to replay and already on top: up to date
        if !inter
            && list.iter().all(|x| x.1 == b'p')
            && self.rr().is_ancestor(onto_c, head)
            && self.rr().range(Some(onto_c), head).len() == list.len()
        {
            let parents_ok = list
                .first()
                .is_none_or(|&(c, _)| self.rr().commits[c].parents.first() == Some(&onto_c));
            if parents_ok {
                let b = self.head_label();
                self.say(OUT, format!("Current branch {b} is up to date."));
                return Ok(());
            }
        }
        if list.first().is_some_and(|x| x.1 == b's' || x.1 == b'f') {
            return Err("error: cannot 'fixup' without a previous commit".into());
        }
        let mut cur = onto_c;
        let mut map: Vec<(usize, usize)> = vec![];
        let mut made: Vec<usize> = vec![];
        for (c, act) in list {
            match act {
                b'd' => {
                    map.push((c, cur));
                }
                b's' | b'f' => {
                    let prev = cur;
                    let k = self.rr().commits[c].clone();
                    let mut base = self.rr().commits[prev].clone();
                    base.fixed |= k.fixed;
                    base.news += k.news;
                    base.rev |= k.rev;
                    base.revnew += k.revnew;
                    base.files |= k.files;
                    let parents = base.parents.clone();
                    let msg = base.msg.clone();
                    let n = self.make(parents, msg, Some(base));
                    let p = self.new_patch();
                    self.r().commits[n].patch = p;
                    for m in map.iter_mut() {
                        if m.1 == prev {
                            m.1 = n;
                        }
                    }
                    map.push((c, n));
                    cur = n;
                    made.push(n);
                }
                _ => {
                    cur = self.copy(c, cur);
                    map.push((c, cur));
                    made.push(cur);
                }
            }
        }
        let name = self.head_label();
        self.r().orig_head = Some(head);
        let onto_id = self.rr().commits[onto_c].id.clone();
        self.r().move_head(
            cur,
            format!("rebase (finish): returning to {name} onto {onto_id}"),
        );
        let mut moved = vec![];
        if update_refs {
            let cur_b = self.rr().cur_branch().map(|s| s.to_string());
            let branches = self.rr().branches.clone();
            for (bname, at) in branches {
                if Some(&bname) == cur_b.as_ref() || self.rr().checked_out(&bname).is_some() {
                    continue;
                }
                if let Some(&(_, to)) = map.iter().find(|m| m.0 == at) {
                    self.r().set_branch(&bname, to);
                    moved.push(format!("\trefs/heads/{bname}"));
                }
            }
        }
        if !self.on_origin {
            self.say(
                OUT,
                format!(
                    "Successfully rebased and updated {}.",
                    if self.rr().cur_branch().is_some() {
                        format!("refs/heads/{name}")
                    } else {
                        "detached HEAD".into()
                    }
                ),
            );
            if !moved.is_empty() {
                self.say(OUT, "Updated the following refs with --update-refs:");
                for m in moved {
                    self.say(OUT, m);
                }
            }
        }
        Ok(())
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

    // ---------- stash ----------

    fn stash_cmd(&mut self, a: &[&str]) -> R<()> {
        let sub = a.first().copied().unwrap_or("push");
        match sub {
            "push" | "save" | "-u" | "--include-untracked" | "-m" => {
                let untracked = a.contains(&"-u") || a.contains(&"--include-untracked");
                let saved: Vec<(usize, bool, bool)> = self
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.staged || (f.modified && (f.tracked || untracked)))
                    .map(|(i, f)| (i, f.staged, f.modified))
                    .collect();
                if saved.is_empty() {
                    self.say(OUT, "No local changes to save");
                    return Ok(());
                }
                for &(i, _, _) in &saved {
                    let f = &mut self.files[i];
                    f.staged = false;
                    f.modified = false;
                    if !f.tracked {
                        f.modified = false;
                    }
                }
                let head = self
                    .repo
                    .head_commit()
                    .map(|c| self.repo.subject(c))
                    .unwrap_or_default();
                let b = self.head_label();
                let what = format!("WIP on {b}: {head}");
                self.say(
                    OUT,
                    format!("Saved working directory and index state {what}"),
                );
                self.stash.insert(0, (what, saved));
                Ok(())
            }
            "pop" | "apply" => {
                if self.stash.is_empty() {
                    return Err("No stash entries found.".into());
                }
                let (_, saved) = self.stash[0].clone();
                for &(i, _, _) in &saved {
                    let f = &mut self.files[i];
                    f.modified = true;
                }
                if sub == "pop" {
                    self.stash.remove(0);
                    self.say(OUT, "Dropped refs/stash@{0}");
                }
                self.status()
            }
            "drop" => {
                if self.stash.is_empty() {
                    return Err("No stash entries found.".into());
                }
                self.stash.remove(0);
                self.say(OUT, "Dropped refs/stash@{0}");
                Ok(())
            }
            "list" => {
                let lines: Vec<String> = self
                    .stash
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("stash@{{{i}}}: {}", s.0))
                    .collect();
                for l in lines {
                    self.say(OUT, l);
                }
                Ok(())
            }
            "show" => {
                let s = self
                    .stash
                    .first()
                    .ok_or("No stash entries found.")?
                    .1
                    .clone();
                for (i, _, _) in s {
                    let n = self.files[i].name.clone();
                    self.say(OUT, format!(" {n}"));
                }
                Ok(())
            }
            "clear" => {
                self.stash.clear();
                Ok(())
            }
            s => Err(format!("error: unknown subcommand: '{s}'")),
        }
    }

    // ---------- bisect ----------

    fn test(&mut self) -> R<()> {
        let Some(h) = self.repo.head_commit() else {
            return Err("make: *** No rule to make target 'test'.".into());
        };
        let bad = self.bug.is_some_and(|b| self.repo.is_ancestor(b, h));
        if bad {
            self.say(FAIL, "FAIL: search finds nothing");
        } else {
            self.say(GOOD, "PASS: 24 tests");
        }
        Ok(())
    }

    fn bisect_cmd(&mut self, a: &[&str]) -> R<()> {
        let sub = a
            .first()
            .copied()
            .ok_or("usage: git bisect start | good | bad | run | reset")?;
        let rest: Vec<&str> = a[1..].to_vec();
        match sub {
            "start" => {
                let orig = self.repo.head().clone();
                self.bisect = Some(Bisect {
                    good: vec![],
                    bad: None,
                    orig,
                    found: None,
                });
                if let Some(b) = rest.first() {
                    let c = self.resolve(b)?;
                    self.bisect.as_mut().unwrap().bad = Some(c);
                    for g in &rest[1..] {
                        let c = self.resolve(g)?;
                        self.bisect.as_mut().unwrap().good.push(c);
                    }
                    self.bisect_next()?;
                } else {
                    self.say(OUT, "status: waiting for both good and bad commits");
                }
                Ok(())
            }
            "good" | "bad" | "old" | "new" => {
                if self.bisect.is_none() {
                    return Err("You need to start by \"git bisect start\"".into());
                }
                let c = match rest.first() {
                    Some(s) => self.resolve(s)?,
                    None => self.repo.head_commit().ok_or("no commit")?,
                };
                let b = self.bisect.as_mut().unwrap();
                if sub == "good" || sub == "old" {
                    b.good.push(c);
                } else {
                    b.bad = Some(c);
                }
                self.bisect_next()
            }
            "run" => {
                if self
                    .bisect
                    .as_ref()
                    .is_none_or(|b| b.bad.is_none() || b.good.is_empty())
                {
                    return Err("git bisect run needs a good and a bad commit first".into());
                }
                if rest.is_empty() {
                    return Err("usage: git bisect run make test".into());
                }
                let mut steps = 0;
                while self.bisect.as_ref().is_some_and(|b| b.found.is_none()) && steps < 20 {
                    steps += 1;
                    let h = self.repo.head_commit().unwrap();
                    let line = format!("running '{}'", rest.join(" "));
                    self.say(OUT, line);
                    self.test()?;
                    let bad = self.bug.is_some_and(|b| self.repo.is_ancestor(b, h));
                    let b = self.bisect.as_mut().unwrap();
                    if bad {
                        b.bad = Some(h);
                    } else {
                        b.good.push(h);
                    }
                    self.bisect_next()?;
                }
                self.say(OUT, "bisect found first bad commit");
                Ok(())
            }
            "reset" => {
                let Some(b) = self.bisect.take() else {
                    self.say(OUT, "We are not bisecting.");
                    return Ok(());
                };
                let from = self.head_label();
                let to = match &b.orig {
                    Head::Branch(n) => n.clone(),
                    Head::Detached(c) => self.repo.commits[*c].id.clone(),
                };
                self.repo.set_head(
                    b.orig.clone(),
                    format!("checkout: moving from {from} to {to}"),
                );
                self.repo.wts[self.repo.cur].prev = None;
                self.say(
                    OUT,
                    format!("Previous HEAD position was {from}\nSwitched to branch '{to}'"),
                );
                Ok(())
            }
            "log" | "visualize" => {
                self.say(OUT, "(the graph shows where you are)");
                Ok(())
            }
            s => Err(format!("error: unknown command: '{s}'")),
        }
    }

    fn bisect_next(&mut self) -> R<()> {
        let b = self.bisect.clone().unwrap();
        let Some(bad) = b.bad else {
            self.say(OUT, "status: waiting for a bad commit, 1 good commit known");
            return Ok(());
        };
        if b.good.is_empty() {
            self.say(OUT, "status: waiting for good commit(s), bad commit known");
            return Ok(());
        }
        let r = &self.repo;
        let mut cand = r.ancestors(bad);
        for &g in &b.good {
            for (k, v) in r.ancestors(g).into_iter().enumerate() {
                if v {
                    cand[k] = false;
                }
            }
        }
        let list: Vec<usize> = (0..r.commits.len()).filter(|&k| cand[k]).collect();
        if list.len() <= 1 {
            let c = list.first().copied().unwrap_or(bad);
            self.bisect.as_mut().unwrap().found = Some(c);
            let s = r.subject(c);
            self.say(
                GOOD,
                format!("{} is the first bad commit\n    {s}", r.commits[c].id),
            );
            return Ok(());
        }
        let n = list.len();
        let mut best = (usize::MAX, bad);
        for &c in &list {
            if c == bad {
                continue;
            }
            let anc = r.ancestors(c);
            let k = list.iter().filter(|&&x| anc[x]).count();
            let d = (2 * k).abs_diff(n);
            if d < best.0 {
                best = (d, c);
            }
        }
        let c = best.1;
        let left = n / 2;
        let steps = (usize::BITS - left.max(1).leading_zeros()) as usize;
        let from = self.head_label();
        let to = self.repo.commits[c].id.clone();
        self.repo.set_head(
            Head::Detached(c),
            format!("checkout: moving from {from} to {to}"),
        );
        let s = self.repo.subject(c);
        self.say(
            OUT,
            format!(
                "Bisecting: {left} revisions left to test after this (roughly {})\n[{s}]",
                plural(steps, "step")
            ),
        );
        Ok(())
    }

    // ---------- worktrees ----------

    fn worktree(&mut self, a: &[&str]) -> R<()> {
        let sub = a
            .first()
            .copied()
            .ok_or("usage: git worktree add | list | remove")?;
        match sub {
            "add" => {
                let mut newb: Option<&str> = None;
                let mut detach = false;
                let mut pos = vec![];
                let mut i = 1;
                while i < a.len() {
                    match a[i] {
                        "-b" | "-B" => {
                            newb = Some(a.get(i + 1).ok_or("error: switch `b' requires a value")?);
                            i += 1;
                        }
                        "--detach" | "-d" => detach = true,
                        s => pos.push(s),
                    }
                    i += 1;
                }
                let path = pos
                    .first()
                    .ok_or("usage: git worktree add <path> [<branch>]")?;
                let name = base_path(path);
                if !valid_name(&name)
                    || name == REPO
                    || self.repo.wts.iter().any(|w| w.path == name)
                {
                    return Err(format!("fatal: '{path}' already exists"));
                }
                let head = if let Some(b) = newb {
                    if self.repo.branch(b).is_some() {
                        return Err(format!("fatal: a branch named '{b}' already exists"));
                    }
                    if !valid_name(b) {
                        return Err(format!("fatal: '{b}' is not a valid branch name"));
                    }
                    let c = match pos.get(1) {
                        Some(s) => self.resolve(s)?,
                        None => self.repo.head_commit().ok_or("fatal: no commits yet")?,
                    };
                    self.repo.set_branch(b, c);
                    Head::Branch(b.to_string())
                } else if let Some(s) = pos.get(1) {
                    if self.repo.branch(s).is_some() && !detach {
                        if let Some(w) = self.repo.checked_out(s) {
                            let p = self.repo.wts[w].path.clone();
                            return Err(format!(
                                "fatal: '{s}' is already used by worktree at '{}'",
                                if w == 0 {
                                    format!("/home/you/{p}")
                                } else {
                                    format!("../{p}")
                                }
                            ));
                        }
                        Head::Branch(s.to_string())
                    } else {
                        Head::Detached(self.resolve(s)?)
                    }
                } else if detach {
                    Head::Detached(self.repo.head_commit().ok_or("fatal: no commits yet")?)
                } else {
                    if self.repo.branch(&name).is_some() {
                        return Err(format!("fatal: a branch named '{name}' already exists"));
                    }
                    let c = self.repo.head_commit().ok_or("fatal: no commits yet")?;
                    self.repo.set_branch(&name, c);
                    Head::Branch(name.clone())
                };
                let desc = match &head {
                    Head::Branch(b) => format!("(new branch or checkout '{b}')"),
                    Head::Detached(c) => format!("(detached HEAD {})", self.repo.commits[*c].id),
                };
                self.repo.wts.push(Worktree {
                    path: name.clone(),
                    head,
                    prev: None,
                });
                self.say(OUT, format!("Preparing worktree {desc}\nHEAD is now in ../{name} (cd ../{name} to work there)"));
                Ok(())
            }
            "list" => {
                let r = &self.repo;
                let lines: Vec<String> = r
                    .wts
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        let at = r
                            .head_of(&w.head)
                            .map(|c| r.commits[c].id.clone())
                            .unwrap_or_default();
                        let what = match &w.head {
                            Head::Branch(b) => format!("[{b}]"),
                            Head::Detached(_) => "(detached HEAD)".into(),
                        };
                        format!(
                            "{:<16} {at} {what}",
                            if i == 0 {
                                format!("/home/you/{}", w.path)
                            } else {
                                format!("/home/you/{}", w.path)
                            }
                        )
                    })
                    .collect();
                for l in lines {
                    self.say(OUT, l);
                }
                Ok(())
            }
            "remove" => {
                let path = a.get(1).ok_or("usage: git worktree remove <path>")?;
                let name = base_path(path);
                let i = self
                    .repo
                    .wts
                    .iter()
                    .position(|w| w.path == name)
                    .ok_or_else(|| format!("fatal: '{path}' is not a working tree"))?;
                if i == 0 {
                    return Err("fatal: '../app' is a main working tree".into());
                }
                if i == self.repo.cur {
                    return Err("fatal: you are inside that worktree; cd ../app first".into());
                }
                self.repo.wts.remove(i);
                if self.repo.cur > i {
                    self.repo.cur -= 1;
                }
                Ok(())
            }
            "prune" => Ok(()),
            s => Err(format!("error: unknown subcommand: '{s}'")),
        }
    }

    fn cd(&mut self, p: &str) -> R<()> {
        let name = base_path(p);
        match self.repo.wts.iter().position(|w| w.path == name) {
            Some(i) => {
                self.repo.cur = i;
                Ok(())
            }
            None if p.is_empty() || p == "~" || p == "." => Ok(()),
            None => Err(format!(
                "cd: {p}: No such directory (worktrees: {})",
                self.repo
                    .wts
                    .iter()
                    .map(|w| format!("../{}", w.path))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    // ---------- origin ----------

    fn clone_to_origin(&mut self) -> R<()> {
        let mut o = self.repo.clone();
        o.remote.clear();
        o.upstream.clear();
        o.reflog.clear();
        o.wts.truncate(1);
        o.cur = 0;
        let branches = self.repo.branches.clone();
        self.repo.remote = branches
            .iter()
            .map(|(n, c)| (format!("origin/{n}"), *c))
            .collect();
        // a fresh clone has only the branch it checked out
        let cur = self.repo.cur_branch().unwrap_or("main").to_string();
        self.repo.branches.retain(|b| b.0 == cur);
        self.repo.upstream = vec![(cur.clone(), cur)];
        self.origin = Some(o);
        Ok(())
    }

    fn need_origin(&self) -> R<()> {
        if self.origin.is_none() {
            return Err("fatal: 'origin' does not appear to be a git repository (this lesson has no remote)".into());
        }
        Ok(())
    }

    fn fetch(&mut self, a: &[&str]) -> R<()> {
        self.need_origin()?;
        let prune = a.contains(&"--prune") || a.contains(&"-p");
        let o = self.origin.clone().unwrap();
        let mut lines = vec![];
        for (name, c) in &o.branches {
            let local = self.repo.import(&o, *c);
            let rname = format!("origin/{name}");
            match self.repo.remote.iter().find(|x| x.0 == rname).map(|x| x.1) {
                Some(old) if old == local => {}
                Some(old) => {
                    let forced = !self.repo.is_ancestor(old, local);
                    lines.push(format!(
                        "  {} {}{}{}  {name} -> {rname}{}",
                        if forced { "+" } else { " " },
                        self.repo.commits[old].id,
                        if forced { "..." } else { ".." },
                        self.repo.commits[local].id,
                        if forced { "  (forced update)" } else { "" }
                    ));
                    Repo::set_ref(&mut self.repo.remote, &rname, local);
                }
                None => {
                    lines.push(format!(" * [new branch]      {name} -> {rname}"));
                    Repo::set_ref(&mut self.repo.remote, &rname, local);
                }
            }
        }
        for (name, c) in &o.tags {
            if self.repo.tag(name).is_none() {
                let local = self.repo.import(&o, *c);
                Repo::set_ref(&mut self.repo.tags, name, local);
                lines.push(format!(" * [new tag]         {name} -> {name}"));
            }
        }
        if prune {
            let gone: Vec<String> = self
                .repo
                .remote
                .iter()
                .filter(|x| o.branch(&x.0[7..]).is_none())
                .map(|x| x.0.clone())
                .collect();
            for g in gone {
                lines.push(format!(" - [deleted]         (none) -> {g}"));
                self.repo.remote.retain(|x| x.0 != g);
            }
        }
        if !lines.is_empty() {
            self.say(OUT, "From origin");
            for l in lines {
                self.say(OUT, l);
            }
        }
        Ok(())
    }

    fn pull(&mut self, a: &[&str]) -> R<()> {
        self.need_origin()?;
        let mut rebase = self.pull_rebase;
        let mut ff_only = false;
        let mut pos = vec![];
        for &s in a {
            match s {
                "--rebase" | "-r" | "--rebase=true" => rebase = Some(true),
                "--no-rebase" | "--rebase=false" => rebase = Some(false),
                "--ff-only" => ff_only = true,
                s if s.starts_with('-') => {}
                s => pos.push(s),
            }
        }
        let b = self
            .repo
            .cur_branch()
            .ok_or("You are not currently on a branch.")?
            .to_string();
        let up = match pos.get(1) {
            Some(s) => s.to_string(),
            None => self.repo.upstream_of(&b).ok_or_else(|| {
                format!("There is no tracking information for the current branch.\nhint: git branch --set-upstream-to=origin/{b} {b}")
            })?,
        };
        self.fetch(&[])?;
        let theirs = self.resolve(&format!("origin/{up}"))?;
        let ours = self.repo.head_commit().ok_or("fatal: no commits yet")?;
        let name = format!("origin/{up}");
        if self.repo.is_ancestor(theirs, ours) {
            self.say(OUT, "Already up to date.");
            return Ok(());
        }
        if self.repo.is_ancestor(ours, theirs) {
            return self.merge_into(ours, theirs, &name, 1, None);
        }
        if ff_only {
            return Err("fatal: Not possible to fast-forward, aborting.".into());
        }
        match rebase {
            Some(true) => self.rebase(&[name.as_str()]),
            Some(false) => self.merge_into(ours, theirs, &name, 1, Some(format!("Merge branch '{up}' of origin"))),
            None => Err("hint: You have divergent branches and need to specify how to reconcile them:\nhint:   git pull --rebase     (your commits go on top)\nhint:   git pull --no-rebase  (a merge commit)\nfatal: Need to specify how to reconcile divergent branches.".into()),
        }
    }

    fn push(&mut self, a: &[&str]) -> R<()> {
        self.need_origin()?;
        let mut force = false;
        let mut lease = false;
        let mut set_up = false;
        let mut delete = false;
        let mut tags = false;
        let mut pos = vec![];
        for &s in a {
            match s {
                "-f" | "--force" => force = true,
                "--force-with-lease" | "--force-if-includes" => lease = true,
                s if s.starts_with("--force-with-lease=") => lease = true,
                "-u" | "--set-upstream" => set_up = true,
                "-d" | "--delete" => delete = true,
                "--tags" | "--follow-tags" => tags = true,
                s if s.starts_with('-') => {}
                s => pos.push(s),
            }
        }
        if pos.first().is_some_and(|p| *p != "origin") {
            return Err(format!(
                "fatal: '{}' does not appear to be a git repository (the remote is called origin)",
                pos[0]
            ));
        }
        let mut specs: Vec<String> = pos.iter().skip(1).map(|s| s.to_string()).collect();
        if specs.is_empty() && !tags {
            let b = self
                .repo
                .cur_branch()
                .ok_or("fatal: You are not currently on a branch.")?
                .to_string();
            match self.repo.upstream_of(&b) {
                Some(up) if up == b => specs.push(b),
                Some(up) => specs.push(format!("{b}:{up}")),
                None if set_up || self.auto_remote => {
                    set_up = true;
                    specs.push(b)
                }
                None => {
                    return Err(format!(
                        "fatal: The current branch {b} has no upstream branch.\nTo push the current branch and set the remote as upstream, use\n\n    git push --set-upstream origin {b}\n"
                    ))
                }
            }
        }
        let mut lines = vec![];
        let mut failed = false;
        for spec in specs {
            if delete || spec.starts_with(':') {
                let name = spec.trim_start_matches(':');
                let o = self.origin.as_mut().unwrap();
                if o.branch(name).is_none() {
                    return Err(format!(
                        "error: unable to delete '{name}': remote ref does not exist"
                    ));
                }
                o.branches.retain(|b| b.0 != name);
                self.repo.remote.retain(|x| x.0 != format!("origin/{name}"));
                lines.push(format!(" - [deleted]         {name}"));
                continue;
            }
            let (src, dst) = match spec.split_once(':') {
                Some((s, d)) => (s.to_string(), d.to_string()),
                None => (
                    spec.clone(),
                    spec.trim_start_matches("refs/heads/").to_string(),
                ),
            };
            if let Some(t) = self.repo.tag(&src) {
                let o = self.origin.as_mut().unwrap();
                if o.tag(&dst).is_none() {
                    let c = o.import(&self.repo, t);
                    Repo::set_ref(&mut o.tags, &dst, c);
                    lines.push(format!(" * [new tag]         {src} -> {dst}"));
                }
                continue;
            }
            let c = self
                .resolve(&src)
                .map_err(|_| format!("error: src refspec {src} does not match any"))?;
            let o = self.origin.clone().unwrap();
            let rname = format!("origin/{dst}");
            let id = self.repo.commits[c].id.clone();
            match o.branch(&dst) {
                Some(old) => {
                    let old_id = o.commits[old].id.clone();
                    let local_old = self.repo.find_id(&old_id);
                    let ff = local_old.is_some_and(|lo| self.repo.is_ancestor(lo, c));
                    if old_id == id {
                        lines.push("Everything up-to-date".into());
                        continue;
                    }
                    if !ff {
                        let seen = self
                            .repo
                            .remote
                            .iter()
                            .find(|x| x.0 == rname)
                            .map(|x| self.repo.commits[x.1].id.clone());
                        if lease && !force && seen.as_deref() != Some(old_id.as_str()) {
                            lines.push(format!(" ! [rejected]        {src} -> {dst} (stale info)"));
                            failed = true;
                            continue;
                        }
                        if !force && !lease {
                            let why = if local_old.is_none() {
                                "fetch first"
                            } else {
                                "non-fast-forward"
                            };
                            lines.push(format!(" ! [rejected]        {src} -> {dst} ({why})"));
                            failed = true;
                            continue;
                        }
                        lines.push(format!(" + {old_id}...{id} {src} -> {dst} (forced update)"));
                    } else {
                        lines.push(format!("   {old_id}..{id}  {src} -> {dst}"));
                    }
                }
                None => lines.push(format!(" * [new branch]      {src} -> {dst}")),
            }
            let oc = self.origin.as_mut().unwrap().import(&self.repo, c);
            self.origin.as_mut().unwrap().set_branch(&dst, oc);
            Repo::set_ref(&mut self.repo.remote, &rname, c);
            if set_up && self.repo.branch(&src).is_some() {
                self.repo.set_upstream(&src, &dst);
                lines.push(format!("branch '{src}' set up to track '{rname}'."));
            }
        }
        if tags {
            let mine = self.repo.tags.clone();
            for (name, t) in mine {
                let o = self.origin.as_mut().unwrap();
                if o.tag(&name).is_none() {
                    let c = o.import(&self.repo, t);
                    Repo::set_ref(&mut o.tags, &name, c);
                    lines.push(format!(" * [new tag]         {name} -> {name}"));
                }
            }
        }
        if lines.iter().any(|l| !l.starts_with("Everything")) {
            self.say(OUT, "To origin");
        }
        for l in lines {
            let k = if l.contains("[rejected]") { ERR } else { OUT };
            self.say(k, l);
        }
        if failed {
            return Err("error: failed to push some refs to 'origin'\nhint: Updates were rejected because the remote has work that you do\nhint: not have locally. Fetch it and integrate it first (git pull).".into());
        }
        Ok(())
    }

    fn remote(&mut self, a: &[&str]) -> R<()> {
        if self.origin.is_none() {
            return Ok(());
        }
        if a.contains(&"-v") {
            self.say(OUT, "origin  git@example.com:team/app.git (fetch)\norigin  git@example.com:team/app.git (push)");
        } else {
            self.say(OUT, "origin");
        }
        Ok(())
    }

    fn config(&mut self, a: &[&str]) -> R<()> {
        let pos: Vec<&str> = a.iter().copied().filter(|s| !s.starts_with('-')).collect();
        let Some(&key) = pos.first() else {
            return Err("usage: git config <name> <value>".into());
        };
        let val = pos.get(1).copied();
        let truth = |v: Option<&str>| -> R<bool> {
            match v {
                Some("true") | Some("yes") | Some("on") | Some("1") => Ok(true),
                Some("false") | Some("no") | Some("off") | Some("0") => Ok(false),
                _ => Err(format!("fatal: bad boolean config value for '{key}'")),
            }
        };
        match key {
            "pull.rebase" => match val {
                None => {
                    let v = self.pull_rebase.map(|b| b.to_string()).unwrap_or_default();
                    self.say(OUT, v);
                }
                v => self.pull_rebase = Some(truth(v)?),
            },
            "push.autoSetupRemote" | "push.autosetupremote" => match val {
                None => {
                    let v = self.auto_remote.to_string();
                    self.say(OUT, v);
                }
                v => self.auto_remote = truth(v)?,
            },
            _ => {}
        }
        Ok(())
    }
}

/// Moves each "fixup! X" and "squash! X" right after the commit X, as
/// git rebase --autosquash does.
fn autosquash_order(r: &Repo, list: Vec<(usize, u8)>) -> Vec<(usize, u8)> {
    let mut out: Vec<(usize, u8)> = vec![];
    let mut later: Vec<(usize, u8)> = vec![];
    for &(c, _) in &list {
        let m = &r.commits[c].msg;
        if m.starts_with("fixup! ") || m.starts_with("squash! ") || m.starts_with("amend! ") {
            later.push((
                c,
                if m.starts_with("squash! ") {
                    b's'
                } else {
                    b'f'
                },
            ));
        } else {
            out.push((c, b'p'));
        }
    }
    for (c, act) in later {
        let m = &r.commits[c].msg;
        let subj = m.split_once("! ").map(|x| x.1).unwrap_or("");
        let subj = subj
            .trim_start_matches("fixup! ")
            .trim_start_matches("squash! ");
        let target = out
            .iter()
            .position(|&(t, _)| r.commits[t].msg == subj || r.commits[t].id.starts_with(subj));
        match target {
            Some(t) => {
                let mut at = t + 1;
                while at < out.len() && out[at].1 != b'p' {
                    at += 1;
                }
                out.insert(at, (c, act));
            }
            None => out.push((c, b'p')),
        }
    }
    out
}

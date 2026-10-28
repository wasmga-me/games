//! git commands: files, commits, history, branches and tags. (Merging,
//! rebasing, remotes and the other tools are in flow.rs.)

use crate::git::{base_name, join, short_among, World, APP, FAIL, GOOD, HINT, HOME, OUT, R};
use crate::lg::{self, Oid, Repo as G};
use crate::vfs;
use std::collections::{HashMap, HashSet};

pub(crate) const NOT_REPO: &str = "fatal: not a git repository (or any of the parent directories): .git";
const NO_WORKTREE: &str = "fatal: this operation must be run in a work tree";

/// The shortest unique id of an object.
pub(crate) fn ab(g: &G, o: &Oid) -> String {
    let all: Vec<Oid> = g.objects().into_iter().map(|x| x.1).collect();
    short_among(o, &all)
}

pub(crate) fn summary(g: &G, o: &Oid) -> String {
    g.commit(o).map(|c| c.summary).unwrap_or_default()
}

/// "Thu Jan 1 00:00:05 2026 +0000"
pub(crate) fn date(t: i64) -> String {
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    // civil from days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let wd = (days + 4).rem_euclid(7) as usize;
    const W: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!(
        "{} {} {} {:02}:{:02}:{:02} {} +0000",
        W[wd],
        M[(m - 1) as usize],
        d,
        secs / 3600,
        secs / 60 % 60,
        secs % 60,
        y
    )
}

pub(crate) fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// Splits "-am" into "-a" "-m": short flags that come together.
pub(crate) fn expand(a: &[&str]) -> Vec<String> {
    let mut v = vec![];
    for (i, w) in a.iter().enumerate() {
        if w.starts_with('-') && !w.starts_with("--") && w.len() > 2 && w.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic()) {
            // -m"msg" and -n5 keep their value
            let chars: Vec<char> = w[1..].chars().collect();
            let mut j = 0;
            while j < chars.len() {
                let c = chars[j];
                v.push(format!("-{c}"));
                if (c == 'm' || c == 'n' || c == 'b' || c == 'B' || c == 'c' || c == 'C') && j + 1 < chars.len() {
                    v.push(chars[j + 1..].iter().collect());
                    break;
                }
                j += 1;
            }
        } else if let Some((k, val)) = w.split_once('=').filter(|_| w.starts_with("--")) {
            v.push(k.to_string());
            v.push(val.to_string());
        } else {
            v.push(w.to_string());
        }
        let _ = i;
    }
    v
}

/// Ids by commit: the names `git log` shows next to them.
pub(crate) fn decorations(g: &G) -> HashMap<Oid, Vec<String>> {
    let mut m: HashMap<Oid, Vec<String>> = HashMap::new();
    let hb = g.head_branch().filter(|_| !g.detached());
    if g.detached() {
        if let Some(h) = g.head() {
            m.entry(h).or_default().push("HEAD".into());
        }
    }
    let refs = g.refs();
    let mut tags = vec![];
    let mut locals = vec![];
    let mut remotes = vec![];
    for r in refs {
        let Ok(o) = g.commit_of(&r) else { continue };
        if let Some(b) = r.strip_prefix("refs/heads/") {
            if Some(b) == hb.as_deref() {
                m.entry(o).or_default().insert(0, format!("HEAD -> {b}"));
            } else {
                locals.push((o, b.to_string()));
            }
        } else if let Some(t) = r.strip_prefix("refs/tags/") {
            tags.push((o, format!("tag: {t}")));
        } else if let Some(x) = r.strip_prefix("refs/remotes/") {
            if !x.ends_with("/HEAD") {
                remotes.push((o, x.to_string()));
            }
        }
    }
    for (o, s) in tags.into_iter().chain(locals).chain(remotes) {
        m.entry(o).or_default().push(s);
    }
    m
}

/// Commits reachable from `from` and not from `hide`, newest first.
pub(crate) fn walk(g: &G, from: &[Oid], hide: &[Oid]) -> Vec<lg::Commit> {
    let reach = |tips: &[Oid]| {
        let mut seen: HashSet<Oid> = HashSet::new();
        let mut stack: Vec<Oid> = tips.to_vec();
        let mut out = vec![];
        while let Some(o) = stack.pop() {
            if !seen.insert(o) {
                continue;
            }
            if let Ok(c) = g.commit(&o) {
                stack.extend(c.parents.iter().copied());
                out.push(c);
            }
        }
        (seen, out)
    };
    let (hidden, _) = reach(hide);
    let (_, mut v) = reach(from);
    v.retain(|c| !hidden.contains(&c.id));
    v.sort_by(|a, b| b.ctime.cmp(&a.ctime).then(b.id.cmp(&a.id)));
    v
}

/// The git commands past init, clone, config, help and version.
pub(crate) const KNOWN: &[&str] = &[
            "status", "add", "rm", "mv", "restore", "commit", "log", "show", "diff", "branch", "switch", "checkout", "merge",
            "rebase", "cherry-pick", "revert", "reset", "tag", "describe", "reflog", "stash", "bisect", "worktree", "fetch",
            "pull", "push", "remote", "rev-parse", "cat-file", "ls-files", "ls-tree", "clean",
];

impl World {
    pub(crate) fn here(&self) -> R<G> {
        lg::discover(&self.cwd).ok_or_else(|| NOT_REPO.to_string())
    }

    /// Paths as libgit2 pathspecs (relative to the working tree); an empty
    /// list means everything.
    pub(crate) fn specs(&self, g: &G, args: &[String]) -> R<Vec<String>> {
        let wd = g.workdir();
        let mut v = vec![];
        for a in args {
            let full = join(&self.cwd, a);
            if full == wd {
                return Ok(vec![]);
            }
            match full.strip_prefix(&format!("{wd}/")) {
                Some(rel) => v.push(rel.to_string()),
                None => return Err(format!("fatal: {a}: '{a}' is outside repository at '{wd}'")),
            }
        }
        Ok(v)
    }

    /// A commit from a revision, with git's message when there is none.
    pub(crate) fn rev(&self, g: &G, spec: &str) -> R<Oid> {
        g.commit_of(spec).map_err(|(code, _)| {
            if code == lg::EUNBORN || (spec == "HEAD" && g.unborn()) {
                "fatal: your current branch has no commits yet".to_string()
            } else {
                format!("fatal: ambiguous argument '{spec}': unknown revision or path not in the working tree.")
            }
        })
    }

    /// Moves the branch you are on (or a detached HEAD) to `to`.
    pub(crate) fn move_head(&self, g: &G, to: &Oid, msg: &str) -> R<()> {
        match g.head_branch().filter(|_| !g.detached()) {
            Some(b) => g.set_ref(&format!("refs/heads/{b}"), to, msg),
            None => g.set_ref("HEAD", to, msg),
        }
    }

    pub(crate) fn git(&mut self, a: &[&str]) -> R<()> {
        let Some(&sub) = a.first() else {
            self.say(OUT, "usage: git <command> [<args>]\nTry git status, git log or git help.");
            return Ok(());
        };
        let rest = &a[1..];
        match sub {
            "init" => return self.init(rest),
            "clone" => return self.clone_cmd(rest),
            "version" | "--version" => {
                self.say(OUT, "git version 2.51.0 (libgit2 1.9.1 inside Branchwise)");
                return Ok(());
            }
            "help" | "--help" | "-h" => {
                self.help();
                return Ok(());
            }
            "config" => return self.config(rest),
            _ => {}
        }
        let unsupported = [
            "blame", "grep", "shortlog", "notes", "submodule", "sparse-checkout", "am", "apply", "format-patch", "archive",
            "gc", "fsck", "prune", "maintenance", "bundle", "filter-branch", "replace", "range-diff", "whatchanged", "mergetool",
            "difftool", "gui", "instaweb", "send-email", "request-pull", "citool", "rerere", "repack", "count-objects",
        ];
        if unsupported.contains(&sub) {
            return Err(format!("git {sub} isn't part of these lessons."));
        }
        if !KNOWN.contains(&sub) {
            let near: Vec<&&str> = KNOWN.iter().filter(|k| distance(k, sub) <= 2).collect();
            if near.is_empty() {
                return Err(format!("git: '{sub}' is not a git command. See 'git --help'."));
            }
            self.err(format!("git: '{sub}' is not a git command. See 'git --help'."));
            let list: Vec<String> = near.iter().map(|k| format!("\t{k}")).collect();
            let one = if near.len() == 1 { "The most similar command is" } else { "The most similar commands are" };
            self.hint(format!("\n{one}\n{}", list.join("\n")));
            return Err(String::new());
        }
        let g = self.here()?;
        match sub {
            "status" => self.status(&g, rest),
            "add" => self.add(&g, rest),
            "rm" => self.rm(&g, rest),
            "mv" => self.mv(&g, rest),
            "restore" => self.restore(&g, rest),
            "commit" => self.commit(&g, rest),
            "log" => self.log(&g, rest),
            "show" => self.show(&g, rest),
            "diff" => self.diff(&g, rest),
            "branch" => self.branch(&g, rest),
            "switch" => self.switch(&g, rest, false),
            "checkout" => self.checkout(&g, rest),
            "merge" => self.merge(&g, rest),
            "rebase" => self.rebase(&g, rest),
            "cherry-pick" => self.pick(&g, rest, false),
            "revert" => self.pick(&g, rest, true),
            "reset" => self.reset(&g, rest),
            "tag" => self.tag(&g, rest),
            "describe" => self.describe(&g, rest),
            "reflog" => self.reflog(&g, rest),
            "stash" => self.stash(&g, rest),
            "bisect" => self.bisect_cmd(&g, rest),
            "worktree" => self.worktree(&g, rest),
            "fetch" => self.fetch(&g, rest),
            "pull" => self.pull(&g, rest),
            "push" => self.push(&g, rest),
            "remote" => self.remote(&g, rest),
            "rev-parse" => self.rev_parse(&g, rest),
            "cat-file" => self.cat_file(&g, rest),
            "ls-files" => self.ls_files(&g, rest),
            "ls-tree" => self.ls_tree(&g, rest),
            "clean" => self.clean(&g, rest),
            _ => unreachable!(),
        }
    }

    pub(crate) fn help(&mut self) {
        self.say(
            OUT,
            "Commands that work here:\n\
             start: init, clone, status, add, rm, mv, restore, commit, config\n\
             history: log, show, diff, describe, reflog, rev-parse, cat-file, ls-files, ls-tree\n\
             branches: branch, switch, checkout, merge, rebase, cherry-pick, revert, reset, tag\n\
             tools: stash, bisect, worktree, clean\n\
             others: remote, fetch, pull, push\n\
             Files: ls, cat, cd, pwd, echo text > file, edit file (adds a line), touch, mkdir, rm, mv, make test",
        );
    }

    // ---------- init ----------

    pub(crate) fn init(&mut self, a: &[&str]) -> R<()> {
        let a = expand(a);
        let mut bare = false;
        let mut branch: Option<String> = None;
        let mut dir: Option<String> = None;
        let mut quiet = false;
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "--bare" => bare = true,
                "-q" | "--quiet" => quiet = true,
                "-b" | "--initial-branch" => {
                    i += 1;
                    branch = a.get(i).cloned();
                }
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => dir = Some(s.to_string()),
            }
            i += 1;
        }
        let path = join(&self.cwd, dir.as_deref().unwrap_or("."));
        let git = if bare { path.clone() } else { format!("{path}/.git") };
        let fs = vfs::cur();
        if fs.exists(&format!("{git}/HEAD")) {
            if !quiet {
                self.say(OUT, format!("Reinitialized existing Git repository in {git}/"));
            }
            return Ok(());
        }
        fs.mkdir_all(&path).map_err(|_| format!("fatal: cannot mkdir {path}"))?;
        let g = lg::create(&path, bare)?;
        if let Some(b) = branch {
            g.set_symref("HEAD", &format!("refs/heads/{b}"), "init")?;
        }
        if !quiet {
            self.say(OUT, format!("Initialized empty Git repository in {git}/"));
        }
        Ok(())
    }

    pub(crate) fn clone_cmd(&mut self, a: &[&str]) -> R<()> {
        let w: Vec<&&str> = a.iter().filter(|x| !x.starts_with('-')).collect();
        let bare = a.contains(&"--bare");
        let Some(url) = w.first() else {
            return Err("fatal: You must specify a repository to clone.".into());
        };
        let src = if url.starts_with('/') || url.starts_with('.') || url.starts_with('~') {
            join(&self.cwd, url)
        } else {
            return Err(format!("fatal: unable to access '{url}': there is no network here; clone a folder such as /srv/origin.git"));
        };
        if !vfs::cur().exists(&src) {
            return Err(format!("fatal: repository '{url}' does not exist"));
        }
        let name = match w.get(1) {
            Some(d) => d.to_string(),
            None => base_name(&src).trim_end_matches(".git").to_string(),
        };
        let dest = join(&self.cwd, &name);
        if vfs::cur().list(&dest).is_some_and(|l| !l.is_empty()) {
            return Err(format!("fatal: destination path '{name}' already exists and is not an empty directory."));
        }
        self.say(OUT, format!("Cloning into '{name}'..."));
        lg::clone(&src, &dest, bare)?;
        self.say(OUT, "done.");
        Ok(())
    }

    // ---------- config ----------

    pub(crate) fn config(&mut self, a: &[&str]) -> R<()> {
        let mut global = false;
        let mut unset = false;
        let mut list = false;
        let mut w = vec![];
        for x in a {
            match *x {
                "--global" => global = true,
                "--local" => global = false,
                "--unset" => unset = true,
                "--get" => {}
                "-l" | "--list" => list = true,
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => w.push(s),
            }
        }
        let file = if global {
            format!("{HOME}/.gitconfig")
        } else {
            match lg::discover(&self.cwd) {
                Some(g) => format!("{}/config", g.commondir()),
                None if w.len() >= 2 || unset => return Err("fatal: not in a git directory".into()),
                None => String::new(),
            }
        };
        if list {
            let mut out = String::new();
            for f in [format!("{HOME}/.gitconfig"), file.clone()] {
                if f.is_empty() {
                    continue;
                }
                if let Some(b) = vfs::cur().read(&f) {
                    out.push_str(&ini_list(&String::from_utf8_lossy(&b)));
                }
            }
            self.say(OUT, out);
            return Ok(());
        }
        let Some(key) = w.first() else {
            return Err("usage: git config [--global] <name> [<value>]".into());
        };
        if !key.contains('.') {
            return Err(format!("error: key does not contain a section: {key}"));
        }
        if unset {
            return lg::set_config_file(&file, key, None).map_err(|_| String::new());
        }
        match w.get(1) {
            Some(v) => lg::set_config_file(&file, key, Some(v)),
            None => {
                let v = match lg::discover(&self.cwd) {
                    Some(g) if !global => g.get_config(key),
                    _ => lg::get_config_file(&format!("{HOME}/.gitconfig"), key),
                };
                match v {
                    Some(v) => {
                        self.say(OUT, v);
                        Ok(())
                    }
                    None => Err(String::new()),
                }
            }
        }
    }

    // ---------- status ----------

    pub(crate) fn status(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        if a.iter().any(|x| x == "-s" || x == "--short" || x == "--porcelain") {
            let mut out = String::new();
            for (f, p) in g.status() {
                out.push_str(&format!("{} {}\n", short_code(f), p));
            }
            self.say(OUT, out);
            return Ok(());
        }
        self.long_status(g);
        Ok(())
    }

    pub(crate) fn long_status(&mut self, g: &G) {
        let mut lines: Vec<(u8, String)> = vec![];
        let mut put = |k: u8, s: String| lines.push((k, s));
        if let Some(b) = &self.bis {
            put(OUT, format!("You are currently bisecting, started from branch '{}'.", b.orig));
        }
        let branch = g.head_branch().filter(|_| !g.detached());
        match (&self.seq, &branch) {
            (Some(s), _) if s.kind == crate::git::SeqKind::Rebase => {
                let onto = ab(g, &s.onto);
                match &s.branch {
                    Some(b) => put(OUT, format!("You are currently rebasing branch '{b}' on '{onto}'.")),
                    None => put(OUT, format!("You are currently rebasing on '{onto}'.")),
                }
            }
            (_, Some(b)) => put(OUT, format!("On branch {b}")),
            _ => match g.head() {
                Some(h) => put(OUT, format!("HEAD detached at {}", ab(g, &h))),
                None => put(OUT, "Not on a branch".into()),
            },
        }
        if let (Some(b), Some(h)) = (&branch, g.head()) {
            if let Some(up) = g.upstream(b) {
                let short = up.trim_start_matches("refs/remotes/").to_string();
                match g.ref_id(&up) {
                    None => put(OUT, format!("Your branch is based on '{short}', but the upstream is gone.")),
                    Some(u) => {
                        let (ahead, behind) = g.ahead_behind(&h, &u);
                        put(
                            OUT,
                            match (ahead, behind) {
                                (0, 0) => format!("Your branch is up to date with '{short}'."),
                                (n, 0) => format!("Your branch is ahead of '{short}' by {}.", plural(n, "commit")),
                                (0, n) => format!("Your branch is behind '{short}' by {}, and can be fast-forwarded.", plural(n, "commit")),
                                (x, y) => format!(
                                    "Your branch and '{short}' have diverged,\nand have {x} and {y} different commits each, respectively."
                                ),
                            },
                        );
                    }
                }
            }
        }
        if g.unborn() {
            put(OUT, "\nNo commits yet".into());
        }
        let st = g.status();
        match g.state() {
            lg::STATE_MERGE => {
                if st.iter().any(|x| x.0 & lg::ST_CONFLICTED != 0) {
                    put(OUT, "You have unmerged paths.".into());
                } else {
                    put(OUT, "All conflicts fixed but you are still merging.".into());
                }
            }
            lg::STATE_CHERRYPICK => put(OUT, "You are currently cherry-picking.".into()),
            lg::STATE_REVERT => put(OUT, "You are currently reverting.".into()),
            _ => {}
        }
        if let Some(s) = &self.seq {
            if s.stopped && s.kind != crate::git::SeqKind::Rebase {
                put(OUT, "You are in the middle of a cherry-pick or revert.".into());
            }
        }
        let staged: Vec<String> = st
            .iter()
            .filter(|x| x.0 & 0x1f != 0 && x.0 & lg::ST_CONFLICTED == 0)
            .map(|(f, p)| {
                let what = if f & lg::ST_INDEX_NEW != 0 {
                    "new file"
                } else if f & lg::ST_INDEX_DELETED != 0 {
                    "deleted"
                } else {
                    "modified"
                };
                format!("\t{what}:   {p}")
            })
            .collect();
        let unmerged: Vec<String> = st
            .iter()
            .filter(|x| x.0 & lg::ST_CONFLICTED != 0)
            .map(|x| format!("\tboth modified:   {}", x.1))
            .collect();
        let changed: Vec<String> = st
            .iter()
            .filter(|x| x.0 & (lg::ST_WT_MODIFIED | lg::ST_WT_DELETED) != 0 && x.0 & lg::ST_CONFLICTED == 0)
            .map(|(f, p)| format!("\t{}:   {p}", if f & lg::ST_WT_DELETED != 0 { "deleted" } else { "modified" }))
            .collect();
        let untracked: Vec<String> = st
            .iter()
            .filter(|x| x.0 & lg::ST_WT_NEW != 0 && x.0 & 0x1f == 0)
            .map(|x| format!("\t{}", x.1))
            .collect();
        if !staged.is_empty() {
            put(OUT, "\nChanges to be committed:".into());
            for s in &staged {
                put(GOOD, s.clone());
            }
        }
        if !unmerged.is_empty() {
            put(OUT, "\nUnmerged paths:".into());
            for s in &unmerged {
                put(FAIL, s.clone());
            }
        }
        if !changed.is_empty() {
            put(OUT, "\nChanges not staged for commit:".into());
            for s in &changed {
                put(FAIL, s.clone());
            }
        }
        if !untracked.is_empty() {
            put(OUT, "\nUntracked files:".into());
            for s in &untracked {
                put(FAIL, s.clone());
            }
        }
        if staged.is_empty() && unmerged.is_empty() {
            put(
                OUT,
                if !changed.is_empty() {
                    "\nno changes added to commit (use \"git add\" and/or \"git commit -a\")".into()
                } else if !untracked.is_empty() {
                    "\nnothing added to commit but untracked files present (use \"git add\" to track)".into()
                } else if g.unborn() {
                    "nothing to commit (create/copy files and use \"git add\" to track)".into()
                } else {
                    "nothing to commit, working tree clean".into()
                },
            );
        }
        for (k, s) in lines {
            self.say(k, s);
        }
    }

    // ---------- the index ----------

    /// Whether a pathspec names something in the files or the index.
    pub(crate) fn matches(&self, g: &G, spec: &str) -> bool {
        if spec.contains(['*', '?', '[']) {
            return true;
        }
        let wd = g.workdir();
        let full = format!("{wd}/{spec}");
        vfs::cur().exists(&full)
            || g.index_entries()
                .iter()
                .any(|e| e.3 == spec || e.3.starts_with(&format!("{spec}/")))
    }

    pub(crate) fn add(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        let mut all = false;
        let mut update = false;
        let mut paths = vec![];
        for x in &a {
            match x.as_str() {
                "-A" | "--all" | "--no-ignore-removal" => all = true,
                "-u" | "--update" => update = true,
                "-v" | "--verbose" | "-f" | "--force" | "--" => {}
                "-p" | "--patch" | "-i" | "--interactive" => {
                    return Err("git add -p needs an editor; add whole files here".into());
                }
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => paths.push(s.to_string()),
            }
        }
        if paths.is_empty() && !all && !update {
            self.say(OUT, "Nothing specified, nothing added.");
            self.hint("hint: Maybe you wanted to say 'git add .'?");
            return Ok(());
        }
        let specs = self.specs(g, &paths)?;
        for (p, s) in paths.iter().zip(&specs) {
            if !self.matches(g, s) {
                return Err(format!("fatal: pathspec '{p}' did not match any files"));
            }
        }
        g.add(&specs, update)?;
        Ok(())
    }

    pub(crate) fn rm(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let cached = a.iter().any(|x| x == "--cached");
        let rec = a.iter().any(|x| x == "-r");
        let quiet = a.iter().any(|x| x == "-q");
        let paths: Vec<String> = a.iter().filter(|x| !x.starts_with('-')).cloned().collect();
        if paths.is_empty() {
            return Err("usage: git rm [--cached] [-r] <file>...".into());
        }
        let specs = self.specs(g, &paths)?;
        let entries = g.index_entries();
        let mut hit: Vec<String> = vec![];
        for (p, s) in paths.iter().zip(&specs) {
            let exact: Vec<String> = entries.iter().filter(|e| e.3 == *s).map(|e| e.3.clone()).collect();
            let under: Vec<String> = entries
                .iter()
                .filter(|e| s.is_empty() || e.3.starts_with(&format!("{s}/")))
                .map(|e| e.3.clone())
                .collect();
            if !exact.is_empty() {
                hit.extend(exact);
            } else if !under.is_empty() {
                if !rec {
                    return Err(format!("fatal: not removing '{p}' recursively without -r"));
                }
                hit.extend(under);
            } else {
                return Err(format!("fatal: pathspec '{p}' did not match any files"));
            }
        }
        hit.dedup();
        let wd = g.workdir();
        for p in hit {
            g.index_remove(&p)?;
            if !cached {
                let _ = vfs::cur().remove(&format!("{wd}/{p}"));
            }
            if !quiet {
                self.say(OUT, format!("rm '{p}'"));
            }
        }
        Ok(())
    }

    pub(crate) fn mv(&mut self, g: &G, a: &[&str]) -> R<()> {
        let w: Vec<String> = a.iter().filter(|x| !x.starts_with('-')).map(|x| x.to_string()).collect();
        if w.len() != 2 {
            return Err("usage: git mv <source> <destination>".into());
        }
        let specs = self.specs(g, &w)?;
        let (from, mut to) = (specs[0].clone(), specs[1].clone());
        let wd = g.workdir();
        let fs = vfs::cur();
        if !g.index_entries().iter().any(|e| e.3 == from) {
            return Err(format!("fatal: not under version control, source={from}, destination={to}"));
        }
        if fs.is_dir(&format!("{wd}/{to}")) {
            to = format!("{to}/{}", base_name(&from));
        }
        if fs.exists(&format!("{wd}/{to}")) {
            return Err(format!("fatal: destination exists, source={from}, destination={to}"));
        }
        fs.rename(&format!("{wd}/{from}"), &format!("{wd}/{to}")).map_err(|_| "fatal: renaming failed".to_string())?;
        g.index_remove(&from)?;
        g.index_add_path(&to)?;
        Ok(())
    }

    pub(crate) fn restore(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        let mut staged = false;
        let mut worktree = false;
        let mut source: Option<String> = None;
        let mut ours = 0u32;
        let mut paths = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "-S" | "--staged" => staged = true,
                "-W" | "--worktree" => worktree = true,
                "-s" | "--source" => {
                    i += 1;
                    source = a.get(i).cloned();
                }
                "--ours" => ours = lg::CHECKOUT_USE_OURS,
                "--theirs" => ours = lg::CHECKOUT_USE_THEIRS,
                "--" => {}
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => paths.push(s.to_string()),
            }
            i += 1;
        }
        if paths.is_empty() {
            return Err("fatal: you must specify path(s) to restore".into());
        }
        if !staged {
            worktree = true;
        }
        let specs = self.specs(g, &paths)?;
        for (p, s) in paths.iter().zip(&specs) {
            if !self.matches(g, s) {
                return Err(format!("error: pathspec '{p}' did not match any file(s) known to git"));
            }
        }
        let src = match &source {
            Some(s) => Some(self.rev(g, s)?),
            None => g.head(),
        };
        if staged {
            g.reset_paths(src.as_ref(), &specs)?;
        }
        if worktree {
            let conflicted = g.conflicts();
            if ours == 0 {
                if let Some(c) = conflicted.iter().find(|c| specs.is_empty() || specs.iter().any(|s| *c == s || c.starts_with(&format!("{s}/")))) {
                    return Err(format!("error: path '{c}' is unmerged"));
                }
            }
            let strategy = lg::CHECKOUT_FORCE | ours;
            let r = if source.is_some() || staged {
                // --staged --worktree, or --source: both from that commit
                g.checkout(src.as_ref(), strategy | if staged { 0 } else { 1 << 8 }, &specs)
            } else {
                g.checkout(None, strategy, &specs)
            };
            r.map_err(|e| e.1)?;
        }
        Ok(())
    }

    // ---------- commit ----------

    pub(crate) fn commit(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        let mut msgs: Vec<String> = vec![];
        let mut all = false;
        let mut amend = false;
        let mut no_edit = false;
        let mut fixup: Option<String> = None;
        let mut allow_empty = false;
        let mut allow_empty_message = false;
        let mut quiet = false;
        let mut paths = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "-m" | "--message" => {
                    i += 1;
                    match a.get(i) {
                        Some(m) => msgs.push(m.clone()),
                        None => return Err("error: switch `m' requires a value".into()),
                    }
                }
                "-a" | "--all" => all = true,
                "--amend" => amend = true,
                "--no-edit" => no_edit = true,
                "--fixup" => {
                    i += 1;
                    fixup = a.get(i).cloned();
                    if fixup.is_none() {
                        return Err("error: option `fixup' requires a value".into());
                    }
                }
                "--allow-empty" => allow_empty = true,
                "--allow-empty-message" => allow_empty_message = true,
                "-q" | "--quiet" => quiet = true,
                "-v" | "--verbose" | "-s" | "--signoff" | "--" => {}
                "-e" | "--edit" => {}
                "-p" | "--patch" | "-i" | "--interactive" => return Err("there is no editor for picking parts of files here".into()),
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => paths.push(s.to_string()),
            }
            i += 1;
        }
        if g.has_conflicts() {
            self.err("error: Committing is not possible because you have unmerged files.");
            self.hint("hint: Fix them up in the work tree, and then use 'git add/rm <file>'\nhint: as appropriate to mark resolution and make a commit.");
            return Err("fatal: Exiting because of an unresolved conflict.".into());
        }
        if !paths.is_empty() {
            let specs = self.specs(g, &paths)?;
            for (p, s) in paths.iter().zip(&specs) {
                if !self.matches(g, s) {
                    return Err(format!("error: pathspec '{p}' did not match any file(s) known to git"));
                }
            }
            g.add(&specs, true)?;
        }
        if all {
            g.add(&[], true)?;
        }
        let head = g.head();
        if amend && head.is_none() {
            return Err("fatal: You have nothing to amend.".into());
        }
        let merging = g.state() == lg::STATE_MERGE;
        let merge_head = if merging { g.ref_id("MERGE_HEAD") } else { None };
        let old = head.and_then(|h| g.commit(&h).ok());
        let parents: Vec<Oid> = if amend {
            old.as_ref().map(|c| c.parents.clone()).unwrap_or_default()
        } else {
            head.into_iter().chain(merge_head).collect()
        };
        let tree = g.write_tree()?;
        let same = match &old {
            Some(c) => c.tree == tree,
            None => g.index_entries().is_empty(),
        };
        if same && !allow_empty && !amend && !merging {
            self.long_status(g);
            return Err(String::new());
        }
        let mut msg = if !msgs.is_empty() {
            msgs.join("\n\n")
        } else if let Some(f) = &fixup {
            let t = self.rev(g, f)?;
            format!("fixup! {}", summary(g, &t))
        } else if amend {
            let _ = no_edit;
            old.as_ref().map(|c| c.msg.clone()).unwrap_or_default()
        } else if merging || g.state() != lg::STATE_NONE {
            g.message().unwrap_or_default()
        } else {
            self.err("Aborting commit due to empty commit message.");
            self.hint("hint: there is no editor here: give the message with -m \"...\"");
            return Err(String::new());
        };
        msg = clean_message(&msg);
        if msg.is_empty() && !allow_empty_message {
            return Err("Aborting commit due to empty commit message.".into());
        }
        let author = if amend {
            old.as_ref().map(|c| (c.author.clone(), c.email.clone(), c.time))
        } else {
            None
        };
        let id = g.commit_tree(
            None,
            &msg,
            &tree,
            &parents,
            author.as_ref().map(|a| (a.0.as_str(), a.1.as_str(), a.2)),
        )?;
        let first = msg.lines().next().unwrap_or("").to_string();
        let kind = if amend {
            "commit (amend)"
        } else if head.is_none() {
            "commit (initial)"
        } else if merging {
            "commit (merge)"
        } else {
            "commit"
        };
        self.move_head(g, &id, &format!("{kind}: {first}"))?;
        if g.state() != lg::STATE_NONE {
            g.cleanup_state();
        }
        if !quiet {
            let where_ = match g.head_branch().filter(|_| !g.detached()) {
                Some(b) => b,
                None => "detached HEAD".into(),
            };
            let root = if parents.is_empty() { " (root-commit)" } else { "" };
            self.say(OUT, format!("[{where_}{root} {}] {first}", ab(g, &id)));
            let stat = g.diff(2, parents.first(), Some(&id), 2, &[]).unwrap_or_default();
            self.say(OUT, stat);
        }
        Ok(())
    }

    // ---------- history ----------

    pub(crate) fn log(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let mut oneline = false;
        let mut all = false;
        let mut limit: Option<usize> = None;
        let mut patch = false;
        let mut stat = false;
        let mut revs: Vec<String> = vec![];
        let mut i = 0;
        while i < a.len() {
            let s = a[i].as_str();
            match s {
                "--oneline" => oneline = true,
                "--all" => all = true,
                "--graph" | "--decorate" | "--no-decorate" | "--abbrev-commit" | "--first-parent" | "--topo-order" | "--date-order" => {}
                "-p" | "--patch" | "-u" => patch = true,
                "--stat" => stat = true,
                "-n" | "--max-count" => {
                    i += 1;
                    limit = a.get(i).and_then(|x| x.parse().ok());
                }
                "--format" | "--pretty" => {
                    i += 1;
                    if a.get(i).is_some_and(|f| f == "oneline") {
                        oneline = true;
                    }
                }
                "--" => break,
                _ if s.starts_with('-') && s[1..].chars().all(|c| c.is_ascii_digit()) && s.len() > 1 => {
                    limit = s[1..].parse().ok();
                }
                _ if s.starts_with('-') => return Err(format!("fatal: unrecognized argument: {s}")),
                _ => revs.push(s.to_string()),
            }
            i += 1;
        }
        let mut from = vec![];
        let mut hide = vec![];
        if all {
            for r in g.refs() {
                if r == "refs/stash" || r.starts_with("refs/bisect") {
                    continue;
                }
                if let Ok(o) = g.commit_of(&r) {
                    from.push(o);
                }
            }
            if let Some(h) = g.head() {
                from.push(h);
            }
        }
        for r in &revs {
            if let Some((x, y)) = r.split_once("..") {
                let x = if x.is_empty() { "HEAD" } else { x };
                let y = if y.is_empty() { "HEAD" } else { y.trim_start_matches('.') };
                hide.push(self.rev(g, x)?);
                from.push(self.rev(g, y)?);
            } else if let Some(x) = r.strip_prefix('^') {
                hide.push(self.rev(g, x)?);
            } else {
                from.push(self.rev(g, r)?);
            }
        }
        if from.is_empty() {
            match g.head() {
                Some(h) => from.push(h),
                None => {
                    let b = g.head_branch().unwrap_or("main".into());
                    return Err(format!("fatal: your current branch '{b}' does not have any commits yet"));
                }
            }
        }
        let list = walk(g, &from, &hide);
        let deco = decorations(g);
        let mut lines: Vec<(u8, String)> = vec![];
        for c in list.iter().take(limit.unwrap_or(usize::MAX)) {
            let d = deco.get(&c.id).map(|v| format!(" ({})", v.join(", "))).unwrap_or_default();
            if oneline {
                lines.push((HINT, format!("{}{d} {}", ab(g, &c.id), c.summary)));
            } else {
                lines.push((HINT, format!("commit {}{d}", c.id.hex())));
                if c.parents.len() > 1 {
                    let ps: Vec<String> = c.parents.iter().map(|p| ab(g, p)).collect();
                    lines.push((OUT, format!("Merge: {}", ps.join(" "))));
                }
                lines.push((OUT, format!("Author: {} <{}>", c.author, c.email)));
                lines.push((OUT, format!("Date:   {}", date(c.time))));
                lines.push((OUT, String::new()));
                for l in c.msg.trim_end().lines() {
                    lines.push((OUT, format!("    {l}")));
                }
                lines.push((OUT, String::new()));
            }
            if patch || stat {
                let t = g.diff(2, c.parents.first(), Some(&c.id), if stat { 1 } else { 0 }, &[]).unwrap_or_default();
                lines.extend(diff_lines(&t));
            }
        }
        if !oneline && lines.last().is_some_and(|l| l.1.is_empty()) {
            lines.pop();
        }
        for (k, s) in lines {
            self.say(k, s);
        }
        Ok(())
    }

    pub(crate) fn show(&mut self, g: &G, a: &[&str]) -> R<()> {
        let stat = a.contains(&"--stat");
        let names: Vec<&&str> = a.iter().filter(|x| !x.starts_with('-')).collect();
        let spec = names.first().map(|s| s.to_string()).unwrap_or("HEAD".into());
        let (id, kind) = g.revparse(&spec).map_err(|_| {
            if spec == "HEAD" && g.unborn() {
                "fatal: your current branch has no commits yet".to_string()
            } else {
                format!("fatal: ambiguous argument '{spec}': unknown revision or path not in the working tree.")
            }
        })?;
        match kind {
            lg::OBJ_BLOB => {
                let b = g.blob(&id).unwrap_or_default();
                self.say(OUT, String::from_utf8_lossy(&b).into_owned());
                return Ok(());
            }
            lg::OBJ_TREE => {
                let mut s = format!("tree {spec}\n\n");
                for (_, k, _, n) in g.ls_tree(&id) {
                    s.push_str(&format!("{n}{}\n", if k == "tree" { "/" } else { "" }));
                }
                self.say(OUT, s);
                return Ok(());
            }
            lg::OBJ_TAG => {
                if let Some((m, _)) = g.tag_info(&id) {
                    self.say(HINT, format!("tag {}", spec));
                    self.say(OUT, format!("\n{}\n", m.trim_end()));
                }
            }
            _ => {}
        }
        let c = g.commit(&self.rev(g, &spec)?)?;
        let deco = decorations(g);
        let d = deco.get(&c.id).map(|v| format!(" ({})", v.join(", "))).unwrap_or_default();
        self.say(HINT, format!("commit {}{d}", c.id.hex()));
        if c.parents.len() > 1 {
            let ps: Vec<String> = c.parents.iter().map(|p| ab(g, p)).collect();
            self.say(OUT, format!("Merge: {}", ps.join(" ")));
        }
        self.say(OUT, format!("Author: {} <{}>\nDate:   {}\n\n", c.author, c.email, date(c.time)));
        for l in c.msg.trim_end().lines() {
            self.say(OUT, format!("    {l}"));
        }
        if c.parents.len() <= 1 {
            self.say(OUT, "");
            let t = g.diff(2, c.parents.first(), Some(&c.id), if stat { 1 } else { 0 }, &[])?;
            for (k, s) in diff_lines(&t) {
                self.say(k, s);
            }
        }
        Ok(())
    }

    pub(crate) fn diff(&mut self, g: &G, a: &[&str]) -> R<()> {
        let mut staged = false;
        let mut what = 0;
        let mut revs: Vec<String> = vec![];
        let mut paths: Vec<String> = vec![];
        let mut dash = false;
        for x in a {
            match *x {
                "--staged" | "--cached" => staged = true,
                "--stat" => what = 1,
                "--name-only" | "--name-status" => what = 3,
                "--" => dash = true,
                s if s.starts_with('-') => return Err(format!("error: invalid option: {s}")),
                s if dash => paths.push(s.to_string()),
                s => {
                    if g.commit_of(s.split("..").next().unwrap_or(s)).is_ok() || s.contains("..") {
                        revs.push(s.to_string());
                    } else if self.matches(g, &self.specs(g, &[s.to_string()])?.first().cloned().unwrap_or_default()) {
                        paths.push(s.to_string());
                    } else {
                        return Err(format!(
                            "fatal: ambiguous argument '{s}': unknown revision or path not in the working tree."
                        ));
                    }
                }
            }
        }
        let specs = self.specs(g, &paths)?;
        let t = if let Some((x, y)) = revs.first().and_then(|r| r.split_once("..")) {
            let x = self.rev(g, if x.is_empty() { "HEAD" } else { x })?;
            let y = self.rev(g, if y.is_empty() { "HEAD" } else { y })?;
            g.diff(2, Some(&x), Some(&y), what, &specs)?
        } else if revs.len() >= 2 {
            let x = self.rev(g, &revs[0])?;
            let y = self.rev(g, &revs[1])?;
            g.diff(2, Some(&x), Some(&y), what, &specs)?
        } else if let Some(r) = revs.first() {
            let x = self.rev(g, r)?;
            g.diff(if staged { 1 } else { 3 }, Some(&x), None, what, &specs)?
        } else if staged {
            let h = g.head();
            g.diff(1, h.as_ref(), None, what, &specs)?
        } else {
            g.diff(0, None, None, what, &specs)?
        };
        for (k, s) in diff_lines(&t) {
            self.say(k, s);
        }
        Ok(())
    }

    pub(crate) fn describe(&mut self, g: &G, a: &[&str]) -> R<()> {
        let tags = a.contains(&"--tags");
        let always = a.contains(&"--always");
        let spec = a.iter().find(|x| !x.starts_with('-')).copied().unwrap_or("HEAD");
        let id = self.rev(g, spec)?;
        match g.describe(&id, tags, always) {
            Ok(s) => {
                self.say(OUT, s);
                Ok(())
            }
            Err(_) => {
                let any_tags = g.refs().iter().any(|r| r.starts_with("refs/tags/"));
                if any_tags && !tags {
                    Err(format!(
                        "fatal: No annotated tags can describe '{}'.\nHowever, there were unannotated tags: try --tags.",
                        id.hex()
                    ))
                } else {
                    Err(format!("fatal: No tags can describe '{}'.\nTry --always, or create some tags.", id.hex()))
                }
            }
        }
    }

    pub(crate) fn reflog(&mut self, g: &G, a: &[&str]) -> R<()> {
        let w: Vec<&&str> = a.iter().filter(|x| !x.starts_with('-') && **x != "show").collect();
        let name = w.first().map(|s| s.to_string()).unwrap_or("HEAD".into());
        let full = if name == "HEAD" {
            name.clone()
        } else if g.ref_id(&format!("refs/heads/{name}")).is_some() {
            format!("refs/heads/{name}")
        } else {
            name.clone()
        };
        let log = g.reflog(&full);
        if log.is_empty() && g.ref_id(&full).is_none() {
            return Err(format!("fatal: ambiguous argument '{name}': unknown revision or path not in the working tree."));
        }
        let deco = decorations(g);
        let mut out = vec![];
        for (i, (o, m)) in log.iter().enumerate() {
            let d = if i == 0 {
                deco.get(o).map(|v| format!(" ({})", v.join(", "))).unwrap_or_default()
            } else {
                String::new()
            };
            out.push(format!("{}{d} {name}@{{{i}}}: {}", ab(g, o), m.trim_end()));
        }
        for l in out {
            self.say(HINT, l);
        }
        Ok(())
    }

    pub(crate) fn rev_parse(&mut self, g: &G, a: &[&str]) -> R<()> {
        let mut short = false;
        let mut abbrev_ref = false;
        for x in a {
            match *x {
                "--short" => short = true,
                "--abbrev-ref" => abbrev_ref = true,
                "--show-toplevel" => self.say(OUT, g.workdir()),
                "--git-dir" => {
                    let d = g.gitdir();
                    self.say(OUT, if d == format!("{}/.git", self.cwd) { ".git".to_string() } else { d });
                }
                "--is-inside-work-tree" => self.say(OUT, (!g.is_bare()).to_string()),
                s if s.starts_with('-') => {}
                s => {
                    if abbrev_ref {
                        let r = g.revparse_ref(s);
                        let n = match r {
                            Some(r) => r
                                .trim_start_matches("refs/heads/")
                                .trim_start_matches("refs/remotes/")
                                .trim_start_matches("refs/tags/")
                                .to_string(),
                            None if s == "HEAD" => "HEAD".into(),
                            None => return Err(format!("fatal: ambiguous argument '{s}': unknown revision")),
                        };
                        self.say(OUT, n);
                        continue;
                    }
                    let (id, _) = g
                        .revparse(s)
                        .map_err(|_| format!("fatal: ambiguous argument '{s}': unknown revision or path not in the working tree."))?;
                    self.say(OUT, if short { ab(g, &id) } else { id.hex() });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn cat_file(&mut self, g: &G, a: &[&str]) -> R<()> {
        let mode = a.iter().find(|x| x.starts_with('-')).copied().unwrap_or("-p");
        let Some(spec) = a.iter().find(|x| !x.starts_with('-')) else {
            return Err("usage: git cat-file (-t | -s | -p) <object>".into());
        };
        let (id, kind) = g.revparse(spec).map_err(|_| format!("fatal: Not a valid object name {spec}"))?;
        let kname = match kind {
            lg::OBJ_COMMIT => "commit",
            lg::OBJ_TREE => "tree",
            lg::OBJ_BLOB => "blob",
            _ => "tag",
        };
        let text = match kind {
            lg::OBJ_COMMIT => {
                let c = g.commit(&id)?;
                let mut s = format!("tree {}\n", c.tree.hex());
                for p in &c.parents {
                    s.push_str(&format!("parent {}\n", p.hex()));
                }
                s.push_str(&format!("author {} <{}> {} +0000\n", c.author, c.email, c.time));
                let me = if c.author == "Sam" { ("Sam", "sam@example.com") } else { ("You", "you@example.com") };
                s.push_str(&format!("committer {} <{}> {} +0000\n\n{}", me.0, me.1, c.ctime, c.msg));
                s
            }
            lg::OBJ_TREE => g
                .ls_tree(&id)
                .into_iter()
                .map(|(m, k, o, n)| format!("{m:06o} {k} {}\t{n}\n", o.hex()))
                .collect(),
            lg::OBJ_BLOB => String::from_utf8_lossy(&g.blob(&id).unwrap_or_default()).into_owned(),
            _ => g.tag_info(&id).map(|(m, t)| format!("object {}\ntype commit\n\n{m}", t.hex())).unwrap_or_default(),
        };
        match mode {
            "-t" => self.say(OUT, kname),
            "-s" => self.say(OUT, text.len().to_string()),
            "-e" => {}
            _ => self.say(OUT, text),
        }
        Ok(())
    }

    pub(crate) fn ls_files(&mut self, g: &G, a: &[&str]) -> R<()> {
        let stage = a.contains(&"-s") || a.contains(&"--stage");
        let mut out = String::new();
        for (st, mode, id, p) in g.index_entries() {
            if stage {
                out.push_str(&format!("{mode:06o} {} {st}\t{p}\n", id.hex()));
            } else {
                out.push_str(&format!("{p}\n"));
            }
        }
        self.say(OUT, out);
        Ok(())
    }

    pub(crate) fn ls_tree(&mut self, g: &G, a: &[&str]) -> R<()> {
        let rec = a.contains(&"-r");
        let Some(spec) = a.iter().find(|x| !x.starts_with('-')) else {
            return Err("usage: git ls-tree [-r] <tree-ish>".into());
        };
        let (id, _) = g.revparse(spec).map_err(|_| format!("fatal: Not a valid object name {spec}"))?;
        let mut out = String::new();
        if rec {
            for (o, p) in g.tree_files(&id) {
                out.push_str(&format!("100644 blob {}\t{p}\n", o.hex()));
            }
        } else {
            for (m, k, o, n) in g.ls_tree(&id) {
                out.push_str(&format!("{m:06o} {k} {}\t{n}\n", o.hex()));
            }
        }
        self.say(OUT, out);
        Ok(())
    }

    pub(crate) fn clean(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let dry = a.iter().any(|x| x == "-n" || x == "--dry-run");
        let force = a.iter().any(|x| x == "-f" || x == "--force");
        let dirs = a.iter().any(|x| x == "-d");
        if !dry && !force {
            return Err("fatal: clean.requireForce is true and -f not given: refusing to clean".into());
        }
        let wd = g.workdir();
        let mut gone = vec![];
        for (f, p) in g.status() {
            if f & lg::ST_WT_NEW != 0 && f & 0x1f == 0 {
                let top = p.split('/').next().unwrap_or(&p).to_string();
                if p.contains('/') && !dirs {
                    continue;
                }
                let show = if p.contains('/') { format!("{top}/") } else { p.clone() };
                if gone.contains(&show) {
                    continue;
                }
                if !dry {
                    let _ = vfs::cur().remove(&format!("{wd}/{}", if p.contains('/') { &top } else { &p }));
                }
                gone.push(show);
            }
        }
        for p in gone {
            self.say(OUT, format!("{} {p}", if dry { "Would remove" } else { "Removing" }));
        }
        Ok(())
    }

    // ---------- branches ----------

    pub(crate) fn branch(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let has = |f: &[&str]| a.iter().any(|x| f.contains(&x.as_str()));
        let pos: Vec<String> = {
            let mut v = vec![];
            let mut i = 0;
            while i < a.len() {
                let x = &a[i];
                if x == "-u" || x == "--set-upstream-to" {
                    i += 2;
                    continue;
                }
                if !x.starts_with('-') {
                    v.push(x.clone());
                }
                i += 1;
            }
            v
        };
        if has(&["--show-current"]) {
            if let Some(b) = g.head_branch().filter(|_| !g.detached()) {
                self.say(OUT, b);
            }
            return Ok(());
        }
        if let Some(i) = a.iter().position(|x| x == "-u" || x == "--set-upstream-to") {
            let up = a.get(i + 1).ok_or("error: option `set-upstream-to' requires a value")?.clone();
            let b = match pos.first() {
                Some(b) => b.clone(),
                None => g.head_branch().ok_or("fatal: could not set upstream of HEAD when it does not point to any branch.")?,
            };
            if g.ref_id(&format!("refs/remotes/{up}")).is_none() && g.ref_id(&format!("refs/heads/{up}")).is_none() {
                return Err(format!("error: the requested upstream branch '{up}' does not exist"));
            }
            g.set_upstream(&b, Some(&up))?;
            self.say(OUT, format!("branch '{b}' set up to track '{up}'."));
            return Ok(());
        }
        if has(&["--unset-upstream"]) {
            let b = pos.first().cloned().or(g.head_branch()).ok_or("fatal: no branch")?;
            g.set_upstream(&b, None).map_err(|_| format!("fatal: branch '{b}' has no upstream information"))?;
            return Ok(());
        }
        if has(&["-d", "-D", "--delete"]) {
            if pos.is_empty() {
                return Err("fatal: branch name required".into());
            }
            let force = has(&["-D", "-f", "--force"]);
            let remote = has(&["-r", "--remotes"]);
            for b in &pos {
                if remote {
                    let id = g.ref_id(&format!("refs/remotes/{b}")).ok_or(format!("error: remote-tracking branch '{b}' not found"))?;
                    g.delete_branch(b, true)?;
                    self.say(OUT, format!("Deleted remote-tracking branch {b} (was {}).", ab(g, &id)));
                    continue;
                }
                let Some(id) = g.ref_id(&format!("refs/heads/{b}")) else {
                    return Err(format!("error: branch '{b}' not found"));
                };
                if let Some(at) = self.checked_out_at(g, b) {
                    return Err(format!("error: cannot delete branch '{b}' used by worktree at '{at}'"));
                }
                if !force {
                    let into = g.upstream(b).and_then(|u| g.ref_id(&u)).or(g.head());
                    let merged = into.is_some_and(|h| h == id || g.descends(&h, &id));
                    if !merged {
                        self.err(format!("error: the branch '{b}' is not fully merged"));
                        self.hint(format!("hint: If you are sure you want to delete it, run 'git branch -D {b}'"));
                        return Err(String::new());
                    }
                }
                g.delete_branch(b, false)?;
                self.say(OUT, format!("Deleted branch {b} (was {}).", ab(g, &id)));
            }
            return Ok(());
        }
        if has(&["-m", "-M", "--move"]) {
            let (old, new) = match pos.len() {
                1 => (g.head_branch().ok_or("fatal: no branch to rename")?, pos[0].clone()),
                2 => (pos[0].clone(), pos[1].clone()),
                _ => return Err("fatal: branch name required".into()),
            };
            if g.ref_id(&format!("refs/heads/{old}")).is_none() && g.head_branch().as_deref() != Some(old.as_str()) {
                return Err(format!("error: refname refs/heads/{old} not found\nfatal: Branch rename failed"));
            }
            if !valid_name(&new) {
                return Err(format!("fatal: '{new}' is not a valid branch name"));
            }
            if g.ref_id(&format!("refs/heads/{new}")).is_some() && !has(&["-M"]) {
                return Err(format!("fatal: a branch named '{new}' already exists"));
            }
            if g.ref_id(&format!("refs/heads/{old}")).is_none() {
                // an unborn branch: only HEAD names it
                g.set_symref("HEAD", &format!("refs/heads/{new}"), "Branch: renamed")?;
                return Ok(());
            }
            g.rename_branch(&old, &new, has(&["-M"]))?;
            return Ok(());
        }
        if has(&["-c", "-C", "--copy"]) {
            return Err("git branch -c isn't part of these lessons; use git branch <new> <old>".into());
        }
        let listing = pos.is_empty() || has(&["--list", "-l"]);
        if listing {
            self.list_branches(g, has(&["-a", "--all"]), has(&["-r", "--remotes"]), a.iter().filter(|x| *x == "-v" || *x == "-vv" || *x == "--verbose").count() + a.iter().filter(|x| *x == "-vv").count());
            return Ok(());
        }
        // create (or with -f move) a branch
        let force = has(&["-f", "--force"]);
        let name = &pos[0];
        if !valid_name(name) {
            return Err(format!("fatal: '{name}' is not a valid branch name"));
        }
        let start = pos.get(1).cloned().unwrap_or("HEAD".into());
        let at = g.commit_of(&start).map_err(|_| {
            if start == "HEAD" {
                format!("fatal: not a valid object name: '{}'", g.head_branch().unwrap_or("main".into()))
            } else {
                format!("fatal: not a valid object name: '{start}'")
            }
        })?;
        let exists = g.ref_id(&format!("refs/heads/{name}")).is_some();
        if exists && !force {
            return Err(format!("fatal: a branch named '{name}' already exists"));
        }
        if exists {
            if let Some(path) = self.checked_out_at(g, name) {
                return Err(format!("fatal: cannot force update the branch '{name}' used by worktree at '{path}'"));
            }
        }
        g.create_branch(name, &at, force)?;
        if g.ref_id(&format!("refs/remotes/{start}")).is_some() && !has(&["--no-track"]) {
            g.set_upstream(name, Some(&start))?;
            self.say(OUT, format!("branch '{name}' set up to track '{start}'."));
        }
        Ok(())
    }

    pub(crate) fn list_branches(&mut self, g: &G, all: bool, remote: bool, verbose: usize) {
        let mut lines: Vec<(u8, String)> = vec![];
        let cur = g.head_branch().filter(|_| !g.detached());
        let refs = g.refs();
        let width = refs
            .iter()
            .filter_map(|r| r.strip_prefix("refs/heads/"))
            .map(|b| b.len())
            .max()
            .unwrap_or(4);
        let detail = |b: &str, id: &Oid| -> String {
            if verbose == 0 {
                return String::new();
            }
            let mut s = format!(" {}", ab(g, id));
            if verbose > 1 {
                if let Some(u) = g.upstream(b) {
                    let short = u.trim_start_matches("refs/remotes/").to_string();
                    let track = match g.ref_id(&u) {
                        None => ": gone".to_string(),
                        Some(uo) => match g.ahead_behind(id, &uo) {
                            (0, 0) => String::new(),
                            (x, 0) => format!(": ahead {x}"),
                            (0, y) => format!(": behind {y}"),
                            (x, y) => format!(": ahead {x}, behind {y}"),
                        },
                    };
                    s.push_str(&format!(" [{short}{track}]"));
                }
            }
            format!("{s} {}", summary(g, id))
        };
        if !remote {
            if g.detached() || self.seq.as_ref().is_some_and(|s| s.kind == crate::git::SeqKind::Rebase) {
                if let Some(h) = g.head() {
                    let what = match self.seq.as_ref().and_then(|s| s.branch.clone()) {
                        Some(b) => format!("(no branch, rebasing {b})"),
                        None => format!("(HEAD detached at {})", ab(g, &h)),
                    };
                    lines.push((GOOD, format!("* {what}{}", if verbose > 0 { format!(" {} {}", ab(g, &h), summary(g, &h)) } else { String::new() })));
                }
            }
            for r in &refs {
                let Some(b) = r.strip_prefix("refs/heads/") else { continue };
                let Some(id) = g.ref_id(r) else { continue };
                let pad = if verbose > 0 { format!("{b:<width$}") } else { b.to_string() };
                if Some(b) == cur.as_deref() && !g.detached() {
                    lines.push((GOOD, format!("* {pad}{}", detail(b, &id))));
                } else if self.checked_out_at(g, b).is_some() {
                    lines.push((HINT, format!("+ {pad}{}", detail(b, &id))));
                } else {
                    lines.push((OUT, format!("  {pad}{}", detail(b, &id))));
                }
            }
        }
        if all || remote {
            for r in &refs {
                let Some(x) = r.strip_prefix("refs/remotes/") else { continue };
                if x.ends_with("/HEAD") {
                    continue;
                }
                let name = if all { format!("remotes/{x}") } else { x.to_string() };
                lines.push((FAIL, format!("  {name}")));
            }
        }
        for (k, s) in lines {
            self.say(k, s);
        }
    }

    /// The folder of the worktree that has `branch` checked out.
    pub(crate) fn checked_out_at(&self, g: &G, branch: &str) -> Option<String> {
        let main = lg::open(&common_workdir(g)).ok()?;
        let mut all = vec![(main.workdir(), main.head_branch().filter(|_| !main.detached()))];
        for (_, p) in main.worktrees() {
            if let Ok(w) = lg::open(&p) {
                all.push((p, w.head_branch().filter(|_| !w.detached())));
            }
        }
        all.into_iter().find(|(_, b)| b.as_deref() == Some(branch)).map(|x| x.0)
    }

    /// Checks out a commit's files, keeping local changes that don't clash;
    /// the error names the files that do.
    pub(crate) fn checkout_files(&mut self, g: &G, to: &Oid, force: bool, what: &str) -> R<()> {
        let strategy = if force { lg::CHECKOUT_FORCE } else { lg::CHECKOUT_SAFE };
        match g.checkout(Some(to), strategy, &[]) {
            Ok(()) => Ok(()),
            Err((files, msg)) => {
                if files.is_empty() {
                    return Err(format!("error: {msg}"));
                }
                let list: Vec<String> = files.iter().map(|f| format!("\t{f}")).collect();
                let tracked: HashSet<String> = g.index_entries().into_iter().map(|e| e.3).collect();
                let untracked = files.iter().all(|f| !tracked.contains(f));
                if untracked {
                    self.err(format!(
                        "error: The following untracked working tree files would be overwritten by {what}:\n{}",
                        list.join("\n")
                    ));
                    self.err("Please move or remove them before you switch branches.\nAborting");
                } else {
                    self.err(format!(
                        "error: Your local changes to the following files would be overwritten by {what}:\n{}",
                        list.join("\n")
                    ));
                    let verb = if what == "checkout" { "switch branches" } else { what };
                    self.err(format!("Please commit your changes or stash them before you {verb}.\nAborting"));
                }
                Err(String::new())
            }
        }
    }

    /// `git switch`, and the branch half of `git checkout`.
    pub(crate) fn switch(&mut self, g: &G, a: &[&str], from_checkout: bool) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        let mut create: Option<(String, bool)> = None;
        let mut detach = false;
        let mut force = false;
        let mut track = true;
        let mut pos: Vec<String> = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "-c" | "--create" | "-b" if !(a[i] == "-b" && !from_checkout) => {
                    i += 1;
                    create = Some((a.get(i).cloned().ok_or("error: switch `c' requires a value")?, false));
                }
                "-C" | "--force-create" | "-B" => {
                    i += 1;
                    create = Some((a.get(i).cloned().ok_or("error: switch `C' requires a value")?, true));
                }
                "-d" | "--detach" => detach = true,
                "-f" | "--force" | "--discard-changes" => force = true,
                "-t" | "--track" => track = true,
                "--no-track" => track = false,
                "--orphan" => return Err("orphan branches aren't part of these lessons".into()),
                "-q" | "--quiet" | "--" => {}
                s if s.starts_with('-') && s != "-" => return Err(format!("error: unknown switch `{}'", s.trim_start_matches('-'))),
                s => pos.push(s.to_string()),
            }
            i += 1;
        }
        let old_head = g.head();
        let old_name = g.head_branch().filter(|_| !g.detached());
        // what to switch to
        if let Some((name, force_create)) = create {
            if !valid_name(&name) {
                return Err(format!("fatal: '{name}' is not a valid branch name"));
            }
            let start = pos.first().cloned().unwrap_or("HEAD".into());
            if g.unborn() && start == "HEAD" {
                g.set_symref("HEAD", &format!("refs/heads/{name}"), "checkout")?;
                self.say(OUT, format!("Switched to a new branch '{name}'"));
                return Ok(());
            }
            let at = g.commit_of(&start).map_err(|_| format!("fatal: invalid reference: {start}"))?;
            let exists = g.ref_id(&format!("refs/heads/{name}")).is_some();
            if exists && !force_create {
                return Err(format!("fatal: a branch named '{name}' already exists"));
            }
            if exists {
                if let Some(p) = self.checked_out_at(g, &name) {
                    return Err(format!("fatal: '{name}' is already used by worktree at '{p}'"));
                }
            }
            self.checkout_files(g, &at, force, "checkout")?;
            g.create_branch(&name, &at, force_create)?;
            if track && g.ref_id(&format!("refs/remotes/{start}")).is_some() {
                g.set_upstream(&name, Some(&start))?;
                self.say(OUT, format!("branch '{name}' set up to track '{start}'."));
            }
            g.set_head(&format!("refs/heads/{name}"))?;
            self.say(OUT, format!("Switched to a {}branch '{name}'", if exists { "and reset " } else { "new " }));
            self.carried(g);
            return Ok(());
        }
        let Some(target) = pos.first().cloned() else {
            return Err(if detach { "fatal: missing branch or commit argument".into() } else { "fatal: missing branch or commit argument".into() });
        };
        if target == "-" || target == "@{-1}" {
            let Some(r) = g.revparse_ref("@{-1}") else {
                return Err("fatal: invalid reference: @{-1}".into());
            };
            let name = r.trim_start_matches("refs/heads/").to_string();
            return self.switch(g, &[name.as_str()], from_checkout);
        }
        let local = g.ref_id(&format!("refs/heads/{target}"));
        if detach || (from_checkout && local.is_none()) {
            let at = g.commit_of(&target).map_err(|_| {
                if from_checkout {
                    format!("error: pathspec '{target}' did not match any file(s) known to git")
                } else {
                    format!("fatal: invalid reference: {target}")
                }
            })?;
            if from_checkout && !detach {
                // `git checkout origin/x` style DWIM for remote branches
                if g.ref_id(&format!("refs/remotes/origin/{target}")).is_some() {
                    return self.switch(g, &[target.as_str()], false);
                }
            }
            self.leaving_behind(g, old_head, at);
            self.checkout_files(g, &at, force, "checkout")?;
            g.set_head_detached(&at)?;
            if from_checkout {
                self.say(OUT, format!("Note: switching to '{target}'.\n\nYou are in 'detached HEAD' state. You can look around, make experimental\nchanges and commit them, and you can discard any commits you make in this\nstate by switching back to a branch."));
            }
            self.say(OUT, format!("HEAD is now at {} {}", ab(g, &at), summary(g, &at)));
            return Ok(());
        }
        let Some(at) = local else {
            // a branch on origin of that name: make a local one tracking it
            if let Some(o) = g.ref_id(&format!("refs/remotes/origin/{target}")) {
                self.checkout_files(g, &o, force, "checkout")?;
                g.create_branch(&target, &o, false)?;
                g.set_upstream(&target, Some(&format!("origin/{target}")))?;
                g.set_head(&format!("refs/heads/{target}"))?;
                self.say(OUT, format!("branch '{target}' set up to track 'origin/{target}'.\nSwitched to a new branch '{target}'"));
                return Ok(());
            }
            return Err(match g.revparse(&target) {
                Ok((_, k)) => {
                    let what = if g.ref_id(&format!("refs/remotes/{target}")).is_some() {
                        "remote branch"
                    } else if g.ref_id(&format!("refs/tags/{target}")).is_some() {
                        "tag"
                    } else if k == lg::OBJ_COMMIT {
                        "commit"
                    } else {
                        "object"
                    };
                    self.err(format!("fatal: a branch is expected, got {what} '{target}'"));
                    self.hint("hint: If you want to detach HEAD at the commit, try again with the --detach option.");
                    String::new()
                }
                Err(_) => format!("fatal: invalid reference: {target}"),
            });
        };
        if old_name.as_deref() == Some(target.as_str()) {
            self.say(OUT, format!("Already on '{target}'"));
            return Ok(());
        }
        if let Some(p) = self.checked_out_at(g, &target) {
            return Err(format!("fatal: '{target}' is already used by worktree at '{p}'"));
        }
        self.leaving_behind(g, old_head, at);
        self.checkout_files(g, &at, force, "checkout")?;
        g.set_head(&format!("refs/heads/{target}"))?;
        self.say(OUT, format!("Switched to branch '{target}'"));
        self.carried(g);
        if let Some(up) = g.upstream(&target) {
            if let Some(u) = g.ref_id(&up) {
                let short = up.trim_start_matches("refs/remotes/");
                let line = match g.ahead_behind(&at, &u) {
                    (0, 0) => format!("Your branch is up to date with '{short}'."),
                    (n, 0) => format!("Your branch is ahead of '{short}' by {}.", plural(n, "commit")),
                    (0, n) => format!("Your branch is behind '{short}' by {}, and can be fast-forwarded.", plural(n, "commit")),
                    (x, y) => format!("Your branch and '{short}' have diverged,\nand have {x} and {y} different commits each, respectively."),
                };
                self.say(OUT, line);
            }
        }
        Ok(())
    }

    /// Lists the local changes a switch carried along.
    pub(crate) fn carried(&mut self, g: &G) {
        for (f, p) in g.status() {
            if f & (lg::ST_WT_MODIFIED | lg::ST_INDEX_MODIFIED) != 0 {
                self.say(OUT, format!("M\t{p}"));
            } else if f & (lg::ST_WT_DELETED | lg::ST_INDEX_DELETED) != 0 {
                self.say(OUT, format!("D\t{p}"));
            } else if f & lg::ST_INDEX_NEW != 0 {
                self.say(OUT, format!("A\t{p}"));
            }
        }
    }

    /// Warns when leaving a detached HEAD whose commits no name holds.
    pub(crate) fn leaving_behind(&mut self, g: &G, old: Option<Oid>, new: Oid) {
        let Some(old) = old else { return };
        if !g.detached() || old == new {
            return;
        }
        let tips: Vec<Oid> = g
            .refs()
            .iter()
            .filter(|r| r.starts_with("refs/heads/") || r.starts_with("refs/tags/") || r.starts_with("refs/remotes/"))
            .filter_map(|r| g.commit_of(r).ok())
            .chain([new])
            .collect();
        let lost = walk(g, &[old], &tips);
        if lost.is_empty() {
            return;
        }
        self.say(
            HINT,
            format!("Warning: you are leaving {} behind, not connected to\nany of your branches:\n", plural(lost.len(), "commit")),
        );
        for c in lost.iter().take(4) {
            self.say(HINT, format!("  {} {}", ab(g, &c.id), c.summary));
        }
        self.say(
            HINT,
            format!("\nIf you want to keep it by creating a new branch, this may be a good time\nto do so with:\n\n git branch <new-branch-name> {}\n", ab(g, &old)),
        );
    }

    pub(crate) fn checkout(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err(NO_WORKTREE.into());
        }
        let a = expand(a);
        let dash = a.iter().position(|x| x == "--");
        let ours = a.iter().any(|x| x == "--ours");
        let theirs = a.iter().any(|x| x == "--theirs");
        let words: Vec<String> = a[..dash.unwrap_or(a.len())].iter().filter(|x| !x.starts_with('-') || *x == "-").cloned().collect();
        let after: Vec<String> = dash.map(|d| a[d + 1..].to_vec()).unwrap_or_default();
        let creates = a.iter().any(|x| x == "-b" || x == "-B");
        // paths: after --, with --ours/--theirs, or a name that is only a path
        let path_mode = !creates
            && (dash.is_some()
                || ours
                || theirs
                || words.first().is_some_and(|w| {
                    g.commit_of(w).is_err()
                        && g.ref_id(&format!("refs/remotes/origin/{w}")).is_none()
                        && w != "-"
                        && self.specs(g, &[w.clone()]).ok().and_then(|s| s.first().cloned().or(Some(String::new()))).is_some_and(|s| self.matches(g, &s))
                }));
        if path_mode {
            let (source, paths) = if dash.is_some() {
                (words.first().cloned(), after)
            } else {
                (None, words.clone())
            };
            if paths.is_empty() {
                return Err("fatal: you must specify path(s) to restore".into());
            }
            let specs = self.specs(g, &paths)?;
            for (p, s) in paths.iter().zip(&specs) {
                if !self.matches(g, s) {
                    return Err(format!("error: pathspec '{p}' did not match any file(s) known to git"));
                }
            }
            let side = if ours {
                lg::CHECKOUT_USE_OURS
            } else if theirs {
                lg::CHECKOUT_USE_THEIRS
            } else {
                0
            };
            match source {
                Some(s) => {
                    let id = self.rev(g, &s)?;
                    g.checkout(Some(&id), lg::CHECKOUT_FORCE, &specs).map_err(|e| e.1)?;
                }
                None => {
                    if side != 0 {
                        let c = g.conflicts();
                        if let Some(p) = specs.iter().find(|p| !c.contains(p)) {
                            return Err(format!("error: path '{p}' does not have {} version", if ours { "our" } else { "their" }));
                        }
                    }
                    if side == 0 {
                        if let Some(c) = g.conflicts().into_iter().find(|c| specs.is_empty() || specs.contains(c)) {
                            return Err(format!("error: path '{c}' is unmerged"));
                        }
                    }
                    g.checkout(None, lg::CHECKOUT_FORCE | side, &specs).map_err(|e| e.1)?;
                }
            }
            let n = specs.len().max(1);
            self.say(OUT, format!("Updated {} from the {}", plural(n, "path"), if ours || theirs { "index" } else { "index" }));
            return Ok(());
        }
        let args: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
        self.switch(g, &args, true)
    }

    pub(crate) fn tag(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let mut msg: Option<String> = None;
        let mut annotate = false;
        let mut delete = false;
        let mut force = false;
        let mut list = false;
        let mut pos: Vec<String> = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "-a" | "--annotate" => annotate = true,
                "-m" | "--message" => {
                    i += 1;
                    msg = a.get(i).cloned();
                    annotate = true;
                }
                "-d" | "--delete" => delete = true,
                "-f" | "--force" => force = true,
                "-l" | "--list" => list = true,
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => pos.push(s.to_string()),
            }
            i += 1;
        }
        if delete {
            for t in &pos {
                let Some(id) = g.ref_id(&format!("refs/tags/{t}")) else {
                    return Err(format!("error: tag '{t}' not found."));
                };
                g.delete_ref(&format!("refs/tags/{t}"))?;
                self.say(OUT, format!("Deleted tag '{t}' (was {})", ab(g, &id)));
            }
            return Ok(());
        }
        if pos.is_empty() || list {
            let mut out = String::new();
            for r in g.refs() {
                if let Some(t) = r.strip_prefix("refs/tags/") {
                    out.push_str(&format!("{t}\n"));
                }
            }
            self.say(OUT, out);
            return Ok(());
        }
        let name = &pos[0];
        if !valid_name(name) {
            return Err(format!("fatal: '{name}' is not a valid tag name."));
        }
        let at = self.rev(g, pos.get(1).map(|s| s.as_str()).unwrap_or("HEAD"))?;
        if g.ref_id(&format!("refs/tags/{name}")).is_some() && !force {
            return Err(format!("fatal: tag '{name}' already exists"));
        }
        if annotate && msg.is_none() {
            self.err("fatal: no tag message?");
            self.hint("hint: there is no editor here: give the message with -m \"...\"");
            return Err(String::new());
        }
        g.tag(name, &at, msg.as_deref().map(|m| clean_message(m)).as_deref(), force)?;
        Ok(())
    }
}

/// The main working tree of the repository `g` is in.
pub(crate) fn common_workdir(g: &G) -> String {
    let c = g.commondir();
    c.strip_suffix("/.git").map(|s| s.to_string()).unwrap_or(if c.is_empty() { APP.to_string() } else { c })
}

/// "XY" as in git status --short.
fn short_code(f: u32) -> String {
    if f & lg::ST_CONFLICTED != 0 {
        return "UU".into();
    }
    if f == lg::ST_WT_NEW {
        return "??".into();
    }
    let x = if f & lg::ST_INDEX_NEW != 0 {
        'A'
    } else if f & lg::ST_INDEX_DELETED != 0 {
        'D'
    } else if f & 0x1f != 0 {
        'M'
    } else {
        ' '
    };
    let y = if f & lg::ST_WT_DELETED != 0 {
        'D'
    } else if f & lg::ST_WT_MODIFIED != 0 {
        'M'
    } else {
        ' '
    };
    format!("{x}{y}")
}

/// Colours for a diff: added lines green, removed ones red.
pub(crate) fn diff_lines(t: &str) -> Vec<(u8, String)> {
    t.lines()
        .map(|l| {
            let k = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") || l.starts_with("index ") {
                OUT
            } else if l.starts_with('+') {
                GOOD
            } else if l.starts_with('-') {
                FAIL
            } else if l.starts_with("@@") {
                HINT
            } else {
                OUT
            };
            (k, l.to_string())
        })
        .collect()
}

pub(crate) fn clean_message(m: &str) -> String {
    let lines: Vec<&str> = m.lines().map(|l| l.trim_end()).filter(|l| !l.starts_with('#')).collect();
    let s = lines.join("\n");
    let s = s.trim();
    if s.is_empty() {
        String::new()
    } else {
        format!("{s}\n")
    }
}

pub(crate) fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with('-')
        && !n.ends_with('/')
        && !n.ends_with(".lock")
        && n != "HEAD"
        && !n.contains("..")
        && !n.contains("@{")
        && !n.contains("//")
        && n.chars().all(|c| c.is_ascii_alphanumeric() || "-_/.+".contains(c))
}

/// key=value lines from a config file.
fn ini_list(t: &str) -> String {
    let mut sec = String::new();
    let mut out = String::new();
    for l in t.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            let inner = l.trim_matches(|c| c == '[' || c == ']');
            sec = match inner.split_once(' ') {
                Some((a, b)) => format!("{}.{}", a, b.trim_matches('"')),
                None => inner.to_string(),
            };
        } else if let Some((k, v)) = l.split_once('=') {
            out.push_str(&format!("{sec}.{}={}\n", k.trim().to_lowercase(), v.trim()));
        }
    }
    out
}


/// How many letters to change, add or remove to turn a into b.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let cur = row[j + 1];
            row[j + 1] = (prev + (ca != b[j]) as usize).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

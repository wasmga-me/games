//! git commands that bring work together or move it between places:
//! merge, rebase, cherry-pick and revert, reset, stash, bisect, worktrees
//! and remotes.

use crate::cmd::{ab, common_workdir, expand, plural, summary, walk};
use crate::git::{base_name, join, BisectState, Seq, SeqKind, Todo, World, FAIL, GOOD, OUT, R};
use crate::lg::{self, Oid, Repo as G};
use crate::vfs;
use std::collections::{HashMap, HashSet};

/// What a refused push or pull leaves for the player.
const PULL_FIRST: &str = "hint: Updates were rejected because the remote contains work that you do not\nhint: have locally. This is usually caused by another repository pushing to\nhint: the same ref. If you want to integrate the remote changes, use\nhint: 'git pull' before pushing again.";
const BEHIND: &str = "hint: Updates were rejected because the tip of your current branch is behind\nhint: its remote counterpart. If you want to integrate the remote changes,\nhint: use 'git pull' before pushing again.";
const DIVERGENT: &str = "hint: You have divergent branches and need to specify how to reconcile them.\nhint: You can do so by running one of the following commands sometime before\nhint: your next pull:\nhint:\nhint:   git config pull.rebase false  # merge\nhint:   git config pull.rebase true   # rebase\nhint:   git config pull.ff only       # fast-forward only\nhint:\nhint: You can also pass --rebase, --no-rebase, or --ff-only on the command\nhint: line to override the configured default per invocation.";

fn stash_index(s: &str) -> R<usize> {
    let n = s
        .strip_prefix("stash@{")
        .and_then(|x| x.strip_suffix('}'))
        .unwrap_or(s);
    n.parse().map_err(|_| format!("error: {s} is not a valid reference"))
}

fn short_ref(r: &str) -> &str {
    r.trim_start_matches("refs/heads/")
        .trim_start_matches("refs/tags/")
        .trim_start_matches("refs/remotes/")
}

impl World {
    fn conflict_lines(&mut self, files: &[String]) {
        for f in files {
            self.say(OUT, format!("Auto-merging {f}"));
            self.say(FAIL, format!("CONFLICT (content): Merge conflict in {f}"));
        }
    }

    /// The message for files a merge would overwrite.
    fn in_the_way(&mut self, files: &[String], msg: &str) -> String {
        if files.is_empty() {
            return format!("error: {msg}");
        }
        let list: Vec<String> = files.iter().map(|f| format!("\t{f}")).collect();
        self.err(format!(
            "error: Your local changes to the following files would be overwritten by merge:\n{}",
            list.join("\n")
        ));
        "Please commit your changes or stash them before you merge.\nAborting".into()
    }

    /// Tracked files with changes (staged or not).
    fn dirty(&self, g: &G) -> (bool, bool) {
        let st = g.status();
        let staged = st.iter().any(|x| x.0 & 0x1f != 0);
        let changed = st.iter().any(|x| x.0 & (lg::ST_WT_MODIFIED | lg::ST_WT_DELETED) != 0);
        (staged, changed)
    }

    fn stat(&mut self, g: &G, a: Option<&Oid>, b: &Oid) {
        let s = g.diff(2, a, Some(b), 1, &[]).unwrap_or_default();
        self.say(OUT, s);
    }

    // ---------- merge ----------

    pub(crate) fn merge(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let a = expand(a);
        let mut no_ff = false;
        let mut ff_only = false;
        let mut squash = false;
        let mut msg: Option<String> = None;
        let mut names = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "--no-ff" => no_ff = true,
                "--ff" => no_ff = false,
                "--ff-only" => ff_only = true,
                "--squash" => squash = true,
                "--abort" => {
                    if g.state() != lg::STATE_MERGE {
                        return Err("fatal: There is no merge to abort (MERGE_HEAD missing).".into());
                    }
                    let h = g.head().ok_or("fatal: no commits yet")?;
                    g.reset(&h, 3)?;
                    g.cleanup_state();
                    return Ok(());
                }
                "--continue" => {
                    if g.state() != lg::STATE_MERGE {
                        return Err("fatal: There is no merge in progress (MERGE_HEAD missing).".into());
                    }
                    return self.commit(g, &[]);
                }
                "-m" | "--message" => {
                    i += 1;
                    msg = Some(a.get(i).cloned().ok_or("error: switch `m' requires a value")?);
                }
                "--no-edit" | "-q" | "--quiet" | "--edit" | "-e" | "--stat" | "--no-stat" => {}
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => names.push(s.to_string()),
            }
            i += 1;
        }
        if g.state() == lg::STATE_MERGE {
            self.err("error: Merging is not possible because you have unmerged files.");
            self.hint("hint: Fix them up in the work tree, and then use 'git add/rm <file>'\nhint: as appropriate to mark resolution and make a commit.");
            return Err("fatal: Exiting because of an unresolved conflict.".into());
        }
        let name = match names.as_slice() {
            [] => {
                let b = g.head_branch().ok_or("fatal: No current branch.")?;
                let up = g
                    .upstream(&b)
                    .ok_or("fatal: No remote for the current branch.")?;
                short_ref(&up).to_string()
            }
            [n] => n.clone(),
            _ => return Err("merging more than one branch at once isn't part of these lessons".into()),
        };
        let theirs = g
            .commit_of(&name)
            .map_err(|_| format!("merge: {name} - not something we can merge"))?;
        let Some(head) = g.head() else {
            // nothing here yet: the branch starts at theirs
            self.checkout_files(g, &theirs, false, "merge")?;
            self.move_head(g, &theirs, &format!("merge {name}: Fast-forward"))?;
            return Ok(());
        };
        if head == theirs || g.descends(&head, &theirs) {
            self.say(OUT, "Already up to date.");
            return Ok(());
        }
        let ff = g.descends(&theirs, &head);
        if ff && !no_ff && !squash {
            self.checkout_files(g, &theirs, false, "merge")?;
            self.move_head(g, &theirs, &format!("merge {name}: Fast-forward"))?;
            self.say(OUT, format!("Updating {}..{}\nFast-forward", ab(g, &head), ab(g, &theirs)));
            self.stat(g, Some(&head), &theirs);
            return Ok(());
        }
        if ff_only {
            return Err("fatal: Not possible to fast-forward, aborting.".into());
        }
        let conflicts = match g.merge(&theirs) {
            Ok(c) => c,
            Err((files, m)) => return Err(self.in_the_way(&files, &m)),
        };
        let here = g.head_branch().filter(|_| !g.detached());
        let mut text = if g.ref_id(&format!("refs/heads/{name}")).is_some() {
            format!("Merge branch '{name}'")
        } else if g.ref_id(&format!("refs/remotes/{name}")).is_some() {
            format!("Merge remote-tracking branch '{name}'")
        } else if g.ref_id(&format!("refs/tags/{name}")).is_some() {
            format!("Merge tag '{name}'")
        } else {
            format!("Merge commit '{name}'")
        };
        if let Some(b) = here.as_ref().filter(|b| *b != "main" && *b != "master") {
            text.push_str(&format!(" into {b}"));
        }
        let text = msg.unwrap_or(text);
        let _ = vfs::cur().write(&format!("{}/MERGE_MSG", g.gitdir()), format!("{text}\n").as_bytes());
        if squash {
            g.cleanup_state();
            self.conflict_lines(&conflicts);
            if !conflicts.is_empty() {
                self.say(FAIL, "Squash commit -- not updating HEAD\nAutomatic merge failed; fix conflicts and then commit the result.");
                return Err(String::new());
            }
            self.say(OUT, "Squash commit -- not updating HEAD\nAutomatic merge went well; stopped before committing as requested");
            return Ok(());
        }
        if !conflicts.is_empty() {
            // markers name the branch, as git writes them
            let wd = g.workdir();
            let fs = vfs::cur();
            for f in &conflicts {
                let p = format!("{wd}/{f}");
                if let Some(b) = fs.read(&p) {
                    let t = String::from_utf8_lossy(&b).replace(&format!(">>>>>>> {}", theirs.hex()), &format!(">>>>>>> {name}"));
                    let _ = fs.write(&p, t.as_bytes());
                }
            }
            self.conflict_lines(&conflicts);
            self.say(FAIL, "Automatic merge failed; fix conflicts and then commit the result.");
            return Err(String::new());
        }
        let id = g.commit_index(None, &text, &[head, theirs], None)?;
        self.move_head(g, &id, &format!("merge {name}: Merge made by the 'ort' strategy."))?;
        g.cleanup_state();
        self.say(OUT, "Merge made by the 'ort' strategy.");
        self.stat(g, Some(&head), &id);
        Ok(())
    }

    // ---------- rebase, cherry-pick, revert ----------

    pub(crate) fn rebase(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let typed = a.iter().take_while(|w| **w != "#todo").copied().collect::<Vec<_>>().join(" ");
        let a = expand(a);
        let mut onto: Option<String> = None;
        let mut inter = false;
        let mut update_refs = g.get_bool("rebase.updateRefs").unwrap_or(false);
        let mut autosquash = false;
        let mut todo: Option<Vec<String>> = None;
        let mut pos: Vec<String> = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "-i" | "--interactive" => inter = true,
                "--onto" => {
                    i += 1;
                    onto = Some(a.get(i).cloned().ok_or("error: option `onto' requires a value")?);
                }
                "--update-refs" => update_refs = true,
                "--no-update-refs" => update_refs = false,
                "--autosquash" => autosquash = true,
                "--no-autosquash" => autosquash = false,
                "--continue" | "--abort" | "--skip" | "--quit" => {
                    return self.seq_control(g, SeqKind::Rebase, &a[i]);
                }
                "#todo" => {
                    todo = Some(a[i + 1..].to_vec());
                    break;
                }
                "-q" | "--quiet" | "-v" | "--merge" | "-m" => {}
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => pos.push(s.to_string()),
            }
            i += 1;
        }
        if self.seq.is_some() {
            return Err("fatal: It seems that there is already a rebase-merge directory, and\nI wonder if you are in the middle of another rebase.  If that is the\ncase, please try\n\tgit rebase (--continue | --abort | --skip)".into());
        }
        let (staged, changed) = self.dirty(g);
        if changed || staged {
            self.err(if changed {
                "error: cannot rebase: You have unstaged changes."
            } else {
                "error: cannot rebase: Your index contains uncommitted changes."
            });
            return Err("error: Please commit or stash them.".into());
        }
        if let Some(b) = pos.get(1).cloned() {
            let q = self.quiet;
            self.quiet = true;
            let r = self.switch(g, &[b.as_str()], true);
            self.quiet = q;
            r?;
        }
        let head = g.head().ok_or("fatal: your current branch has no commits yet")?;
        let branch = g.head_branch().filter(|_| !g.detached());
        let upstream_name = match pos.first() {
            Some(s) => s.clone(),
            None => {
                let up = branch.as_ref().and_then(|b| g.upstream(b)).ok_or(
                    "There is no tracking information for the current branch.\nPlease specify which branch you want to rebase against.",
                )?;
                short_ref(&up).to_string()
            }
        };
        let upstream = g
            .commit_of(&upstream_name)
            .map_err(|_| format!("fatal: invalid upstream '{upstream_name}'"))?;
        let onto_name = onto.clone().unwrap_or(upstream_name.clone());
        let onto_id = g
            .commit_of(&onto_name)
            .map_err(|_| format!("fatal: invalid upstream '{onto_name}'"))?;
        // the commits to replay, oldest first; merges and changes already
        // on the new base are left out
        let mut mine: Vec<lg::Commit> = walk(g, &[head], &[upstream]);
        mine.reverse();
        let upstream_patches: HashSet<Oid> = walk(g, &[onto_id], &[head])
            .iter()
            .filter_map(|c| g.patch_id(&c.id))
            .collect();
        let replay: Vec<&lg::Commit> = mine
            .iter()
            .filter(|c| c.parents.len() < 2)
            .filter(|c| g.patch_id(&c.id).map_or(true, |p| !upstream_patches.contains(&p)))
            .collect();
        let mut list: Vec<(u8, Oid)> = replay.iter().map(|c| (b'p', c.id)).collect();
        if autosquash {
            list = autosquash_order(g, list);
        }
        if inter && todo.is_none() {
            if list.is_empty() {
                return Err("error: nothing to do".into());
            }
            self.todo = Some(Todo {
                cmd: format!("git rebase {typed}"),
                lines: list
                    .iter()
                    .enumerate()
                    .map(|(k, (act, o))| (self.repo.index_of(o).unwrap_or(0), *act, k))
                    .collect(),
                sel: 0,
            });
            return Ok(());
        }
        if let Some(t) = todo {
            let mut l2 = vec![];
            for e in &t {
                let (act, k) = e.split_at(1);
                let k: usize = k.parse().map_err(|_| "error: the to-do list is broken")?;
                let c = list.get(k).ok_or("error: the to-do list is broken")?.1;
                if act != "d" {
                    l2.push((act.as_bytes()[0], c));
                }
            }
            if l2.first().is_some_and(|x| x.0 != b'p') {
                return Err("error: cannot 'fixup' without a previous commit".into());
            }
            list = l2;
        } else if !inter {
            // already on top, nothing to change
            let mut chain = vec![];
            let mut at = head;
            while at != onto_id {
                let Ok(c) = g.commit(&at) else { break };
                if c.parents.len() != 1 {
                    break;
                }
                chain.push(at);
                at = c.parents[0];
            }
            chain.reverse();
            let same: Vec<Oid> = list.iter().map(|x| x.1).collect();
            if at == onto_id && chain == same && list.iter().all(|x| x.0 == b'p') {
                match &branch {
                    Some(b) => self.say(OUT, format!("Current branch {b} is up to date.")),
                    None => self.say(OUT, "HEAD is up to date."),
                }
                return Ok(());
            }
        }
        // branches inside the replayed commits
        let mut moved = vec![];
        if update_refs {
            let inside: HashSet<Oid> = list.iter().map(|x| x.1).collect();
            for r in g.refs() {
                if let Some(b) = r.strip_prefix("refs/heads/") {
                    if Some(b) == branch.as_deref() {
                        continue;
                    }
                    if let Some(o) = g.ref_id(&r) {
                        if inside.contains(&o) && self.checked_out_at(g, b).is_none() {
                            moved.push((b.to_string(), o));
                        }
                    }
                }
            }
        }
        g.checkout(Some(&onto_id), lg::CHECKOUT_FORCE, &[]).map_err(|e| e.1)?;
        g.set_ref("ORIG_HEAD", &head, "rebase")?;
        g.set_head_detached(&onto_id)?;
        self.seq = Some(Seq {
            kind: SeqKind::Rebase,
            steps: list,
            at: 0,
            onto: onto_id,
            orig: head,
            branch,
            update_refs: moved,
            rewritten: vec![],
            stopped: false,
            mainline: 0,
        });
        self.run_seq(g)
    }

    pub(crate) fn pick(&mut self, g: &G, a: &[&str], revert: bool) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let kind = if revert { SeqKind::Revert } else { SeqKind::Pick };
        let name = if revert { "revert" } else { "cherry-pick" };
        let a = expand(a);
        let mut mainline = 0u32;
        let mut revs: Vec<String> = vec![];
        let mut i = 0;
        while i < a.len() {
            match a[i].as_str() {
                "--continue" | "--abort" | "--skip" | "--quit" => return self.seq_control(g, kind, &a[i]),
                "-m" | "--mainline" => {
                    i += 1;
                    mainline = a
                        .get(i)
                        .and_then(|x| x.parse().ok())
                        .ok_or("error: option `mainline' expects a number greater than zero")?;
                }
                "-x" | "--no-edit" | "-e" | "--edit" | "--ff" => {}
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => revs.push(s.to_string()),
            }
            i += 1;
        }
        if revs.is_empty() {
            return Err(format!("usage: git {name} [<options>] <commit-ish>..."));
        }
        if self.seq.is_some() || g.state() != lg::STATE_NONE {
            self.err(format!("error: {name} is already in progress"));
            return Err(format!("hint: try \"git {name} (--continue | --abort | --quit)\""));
        }
        let head = g.head().ok_or("fatal: your current branch has no commits yet")?;
        let mut steps = vec![];
        for r in &revs {
            if let Some((x, y)) = r.split_once("..") {
                let from = self.rev(g, if x.is_empty() { "HEAD" } else { x })?;
                let to = self.rev(g, if y.is_empty() { "HEAD" } else { y })?;
                let mut v = walk(g, &[to], &[from]);
                if !revert {
                    v.reverse();
                }
                steps.extend(v.into_iter().map(|c| (b'p', c.id)));
            } else {
                steps.push((b'p', self.rev(g, r)?));
            }
        }
        for (_, c) in &steps {
            let k = g.commit(c)?;
            if k.parents.len() > 1 && mainline == 0 {
                return Err(format!("error: commit {} is a merge but no -m option was given.\nfatal: {name} failed", c.hex()));
            }
        }
        let (staged, changed) = self.dirty(g);
        if staged || changed {
            self.err("error: your local changes would be overwritten by cherry-pick.");
            self.hint("hint: commit your changes or stash them to proceed.");
            return Err(format!("fatal: {name} failed"));
        }
        self.seq = Some(Seq {
            kind,
            steps,
            at: 0,
            onto: head,
            orig: head,
            branch: g.head_branch().filter(|_| !g.detached()),
            update_refs: vec![],
            rewritten: vec![],
            stopped: false,
            mainline,
        });
        self.run_seq(g)
    }

    /// --continue, --skip, --abort and --quit.
    fn seq_control(&mut self, g: &G, kind: SeqKind, what: &str) -> R<()> {
        let name = match kind {
            SeqKind::Rebase => "rebase",
            SeqKind::Pick => "cherry-pick",
            SeqKind::Revert => "revert",
        };
        let Some(s) = self.seq.clone().filter(|s| s.kind == kind) else {
            return Err(match kind {
                SeqKind::Rebase => "fatal: No rebase in progress?".into(),
                _ => format!("error: no {name} in progress"),
            });
        };
        match what {
            "--abort" => {
                g.checkout(Some(&s.orig), lg::CHECKOUT_FORCE, &[]).map_err(|e| e.1)?;
                match &s.branch {
                    Some(b) => {
                        if kind != SeqKind::Rebase {
                            g.set_ref(&format!("refs/heads/{b}"), &s.orig, &format!("{name}: aborting"))?;
                        }
                        g.set_head(&format!("refs/heads/{b}"))?;
                    }
                    None => g.set_head_detached(&s.orig)?,
                }
                g.read_tree(&s.orig)?;
                g.cleanup_state();
                self.seq = None;
                Ok(())
            }
            "--quit" => {
                g.cleanup_state();
                self.seq = None;
                Ok(())
            }
            "--skip" => {
                let h = g.head().ok_or("fatal: no commits yet")?;
                g.checkout(Some(&h), lg::CHECKOUT_FORCE, &[]).map_err(|e| e.1)?;
                g.read_tree(&h)?;
                g.cleanup_state();
                let s = self.seq.as_mut().unwrap();
                s.at += 1;
                s.stopped = false;
                self.run_seq(g)
            }
            _ => {
                if g.has_conflicts() {
                    self.err("error: you must edit all merge conflicts and then\nmark them as resolved using git add");
                    return Err(String::new());
                }
                if self.dirty(g).1 {
                    self.err(format!(
                        "error: you have unstaged changes in your working tree. Stage them with git add, then run \"git {name} --continue\" again."
                    ));
                    return Err(String::new());
                }
                if s.stopped {
                    let (act, c) = s.steps[s.at];
                    self.finish_step(g, act, &c, true)?;
                    let s = self.seq.as_mut().unwrap();
                    s.at += 1;
                    s.stopped = false;
                }
                self.run_seq(g)
            }
        }
    }

    /// Applies the steps left; stops at a conflict.
    fn run_seq(&mut self, g: &G) -> R<()> {
        loop {
            let Some(s) = self.seq.clone() else { return Ok(()) };
            if s.at >= s.steps.len() {
                break;
            }
            let (act, c) = s.steps[s.at];
            let k = g.commit(&c)?;
            let revert = s.kind == SeqKind::Revert;
            match g.cherrypick(&c, s.mainline, revert) {
                Err((files, m)) => {
                    let e = self.in_the_way(&files, &m);
                    self.seq.as_mut().unwrap().stopped = true;
                    return Err(e);
                }
                Ok(conflicts) if !conflicts.is_empty() => {
                    self.conflict_lines(&conflicts);
                    self.seq.as_mut().unwrap().stopped = true;
                    let verb = if revert { "revert" } else { "apply" };
                    self.say(FAIL, format!("error: could not {verb} {}... {}", ab(g, &c), k.summary));
                    let name = match s.kind {
                        SeqKind::Rebase => {
                            g.cleanup_state();
                            self.hint("hint: Resolve all conflicts manually, mark them as resolved with\nhint: \"git add/rm <conflicted_files>\", then run \"git rebase --continue\".\nhint: You can instead skip this commit: run \"git rebase --skip\".\nhint: To abort and get back to the state before \"git rebase\", run \"git rebase --abort\".");
                            return Err(String::new());
                        }
                        SeqKind::Pick => "cherry-pick",
                        SeqKind::Revert => "revert",
                    };
                    self.hint(format!("hint: After resolving the conflicts, mark them with\nhint: \"git add/rm <pathspec>\", then run\nhint: \"git {name} --continue\".\nhint: You can instead skip this commit with \"git {name} --skip\".\nhint: To abort and get back to the state before \"git {name}\",\nhint: run \"git {name} --abort\"."));
                    return Err(String::new());
                }
                Ok(_) => {
                    if let Err(e) = self.finish_step(g, act, &c, false) {
                        self.seq.as_mut().unwrap().stopped = true;
                        return Err(e);
                    }
                    self.seq.as_mut().unwrap().at += 1;
                }
            }
        }
        let s = self.seq.take().unwrap();
        g.cleanup_state();
        if s.kind != SeqKind::Rebase {
            return Ok(());
        }
        let last = g.head().unwrap_or(s.onto);
        let new_of = |o: &Oid| -> Oid {
            // a commit that was dropped or folded goes with the copy before it
            let mut best = s.onto;
            for (old, new) in &s.rewritten {
                best = *new;
                if old == o {
                    break;
                }
            }
            best
        };
        let mut updated = vec![];
        for (b, old) in &s.update_refs {
            let to = new_of(old);
            g.set_ref(&format!("refs/heads/{b}"), &to, "rebase (update-refs)")?;
            updated.push(format!("\trefs/heads/{b}"));
        }
        match &s.branch {
            Some(b) => {
                let full = format!("refs/heads/{b}");
                g.set_ref(&full, &last, &format!("rebase (finish): {full} onto {}", s.onto.hex()))?;
                g.set_head(&full)?;
                self.say(GOOD, format!("Successfully rebased and updated {full}."));
            }
            None => self.say(GOOD, "Successfully rebased and updated detached HEAD."),
        }
        if !updated.is_empty() {
            self.say(OUT, format!("Updated the following refs with --update-refs:\n{}", updated.join("\n")));
        }
        Ok(())
    }

    /// Commits what a pick left in the index.
    fn finish_step(&mut self, g: &G, act: u8, c: &Oid, continued: bool) -> R<()> {
        let s = self.seq.clone().unwrap();
        let k = g.commit(c)?;
        let head = g.head().ok_or("fatal: no commits yet")?;
        let h = g.commit(&head)?;
        let tree = g.write_tree()?;
        let rebase = s.kind == SeqKind::Rebase;
        if tree == h.tree && act == b'p' {
            g.cleanup_state();
            if rebase || continued {
                return Ok(());
            }
            self.err("The previous cherry-pick is now empty, possibly due to conflict resolution.");
            self.hint("hint: To skip it, run \"git cherry-pick --skip\".");
            return Err(String::new());
        }
        let (msg, parents, author) = match (s.kind, act) {
            (SeqKind::Revert, _) => (
                format!("Revert \"{}\"\n\nThis reverts commit {}.\n", k.summary, c.hex()),
                vec![head],
                None,
            ),
            (_, b's') => (
                format!("{}\n\n{}", h.msg.trim_end(), k.msg),
                h.parents.clone(),
                Some((h.author.clone(), h.email.clone(), h.time)),
            ),
            (_, b'f') => (h.msg.clone(), h.parents.clone(), Some((h.author.clone(), h.email.clone(), h.time))),
            _ => (k.msg.clone(), vec![head], Some((k.author.clone(), k.email.clone(), k.time))),
        };
        let id = g.commit_tree(None, &msg, &tree, &parents, author.as_ref().map(|a| (a.0.as_str(), a.1.as_str(), a.2)))?;
        let first = msg.lines().next().unwrap_or("").to_string();
        if rebase {
            let what = match act {
                b's' => "squash",
                b'f' => "fixup",
                _ => "pick",
            };
            g.set_ref("HEAD", &id, &format!("rebase ({what}): {first}"))?;
            let seq = self.seq.as_mut().unwrap();
            if act != b'p' {
                for r in seq.rewritten.iter_mut() {
                    if r.1 == head {
                        r.1 = id;
                    }
                }
            }
            seq.rewritten.push((*c, id));
        } else {
            let name = if s.kind == SeqKind::Revert { "revert" } else { "cherry-pick" };
            self.move_head(g, &id, &format!("{name}: {first}"))?;
            let where_ = g.head_branch().filter(|_| !g.detached()).unwrap_or("detached HEAD".into());
            self.say(OUT, format!("[{where_} {}] {first}", ab(g, &id)));
            let st = g.diff(2, Some(&head), Some(&id), 2, &[]).unwrap_or_default();
            self.say(OUT, st);
        }
        g.cleanup_state();
        Ok(())
    }

    // ---------- reset ----------

    pub(crate) fn reset(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let a = expand(a);
        let mut kind = 2;
        let mut quiet = false;
        let mut words: Vec<String> = vec![];
        let mut paths: Vec<String> = vec![];
        let mut after_dash = false;
        for w in &a {
            if after_dash {
                paths.push(w.clone());
                continue;
            }
            match w.as_str() {
                "--soft" => kind = 1,
                "--mixed" => kind = 2,
                "--hard" => kind = 3,
                "--keep" | "--merge" => kind = 3,
                "-q" | "--quiet" => quiet = true,
                "--" => after_dash = true,
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => words.push(s.to_string()),
            }
        }
        // `git reset file`: a word that is no revision is a path
        if paths.is_empty() && !words.is_empty() && g.commit_of(&words[0]).is_err() {
            paths = std::mem::take(&mut words);
        } else if words.len() > 1 {
            paths.extend(words.drain(1..));
        }
        if !paths.is_empty() {
            if kind == 3 {
                return Err("fatal: Cannot do hard reset with paths.".into());
            }
            if kind == 1 {
                return Err("fatal: Cannot do soft reset with paths.".into());
            }
            let specs = self.specs(g, &paths)?;
            let to = match words.first() {
                Some(r) => Some(self.rev(g, r)?),
                None => g.head(),
            };
            g.reset_paths(to.as_ref(), &specs)?;
            if !quiet {
                self.unstaged_after(g);
            }
            return Ok(());
        }
        let spec = words.first().cloned().unwrap_or("HEAD".into());
        let to = self.rev(g, &spec)?;
        let old = g.head();
        let msg = format!("reset: moving to {spec}");
        if kind == 3 {
            g.checkout(Some(&to), lg::CHECKOUT_FORCE, &[]).map_err(|e| e.1)?;
        }
        if let Some(o) = old {
            g.set_ref("ORIG_HEAD", &o, "reset")?;
        }
        self.move_head(g, &to, &msg)?;
        if kind >= 2 {
            g.read_tree(&to)?;
            g.cleanup_state();
        }
        if quiet {
            return Ok(());
        }
        if kind == 3 {
            self.say(OUT, format!("HEAD is now at {} {}", ab(g, &to), summary(g, &to)));
        } else if kind == 2 {
            self.unstaged_after(g);
        }
        Ok(())
    }

    fn unstaged_after(&mut self, g: &G) {
        let v: Vec<String> = g
            .status()
            .into_iter()
            .filter(|(f, _)| f & (lg::ST_WT_MODIFIED | lg::ST_WT_DELETED) != 0)
            .map(|(f, p)| format!("{}\t{p}", if f & lg::ST_WT_DELETED != 0 { 'D' } else { 'M' }))
            .collect();
        if !v.is_empty() {
            self.say(OUT, format!("Unstaged changes after reset:\n{}", v.join("\n")));
        }
    }

    // ---------- stash ----------

    pub(crate) fn stash(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let a = expand(a);
        let (sub, rest): (&str, &[String]) = match a.first().map(|s| s.as_str()) {
            Some(s) if !s.starts_with('-') => (s, &a[1..]),
            _ => ("push", &a[..]),
        };
        let pick = |rest: &[String]| -> R<usize> {
            match rest.iter().find(|x| !x.starts_with('-')) {
                Some(s) => stash_index(s),
                None => Ok(0),
            }
        };
        match sub {
            "push" | "save" => {
                let mut msg: Option<String> = None;
                let mut untracked = false;
                let mut i = 0;
                let mut words = vec![];
                while i < rest.len() {
                    match rest[i].as_str() {
                        "-m" | "--message" => {
                            i += 1;
                            msg = rest.get(i).cloned();
                        }
                        "-u" | "--include-untracked" | "-a" | "--all" => untracked = true,
                        "-q" | "--quiet" | "-k" | "--keep-index" => {}
                        "-p" | "--patch" => return Err("there is no editor for picking parts of files here".into()),
                        s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                        s => words.push(s.to_string()),
                    }
                    i += 1;
                }
                if sub == "save" && !words.is_empty() {
                    msg = Some(words.join(" "));
                } else if !words.is_empty() {
                    return Err("stashing only some files isn't part of these lessons".into());
                }
                if g.unborn() {
                    return Err("You do not have the initial commit yet".into());
                }
                let st = g.status();
                let any = st
                    .iter()
                    .any(|(f, _)| (*f != lg::ST_WT_NEW || untracked) && *f != 0);
                if !any {
                    self.say(OUT, "No local changes to save");
                    return Ok(());
                }
                if g.has_conflicts() {
                    return Err("error: could not write index\nYou have unmerged paths; fix them first.".into());
                }
                let branch = g.head_branch().filter(|_| !g.detached()).unwrap_or("(no branch)".into());
                let head = g.head().unwrap();
                let text = match &msg {
                    Some(m) => format!("On {branch}: {m}"),
                    None => format!("WIP on {branch}: {} {}", ab(g, &head), summary(g, &head)),
                };
                g.stash_save(Some(&text), untracked)?;
                self.say(OUT, format!("Saved working directory and index state {text}"));
                Ok(())
            }
            "list" => {
                for (i, _, m) in g.stashes() {
                    self.say(OUT, format!("stash@{{{i}}}: {m}"));
                }
                Ok(())
            }
            "pop" | "apply" => {
                let i = pick(rest)?;
                let list = g.stashes();
                let Some(e) = list.iter().find(|x| x.0 == i).cloned() else {
                    return Err(if list.is_empty() {
                        "No stash entries found.".into()
                    } else {
                        format!("error: stash@{{{i}}} is not a valid reference")
                    });
                };
                let index = rest.iter().any(|x| x == "--index");
                match g.stash_apply(i, sub == "pop", index) {
                    Err((files, m)) => {
                        if !files.is_empty() {
                            return Err(self.in_the_way(&files, &m));
                        }
                        if m.contains("conflict") {
                            self.err("error: the stash clashes with changes here; commit or restore them, then try again.");
                            return Err("The stash entry is kept in case you need it again.".into());
                        }
                        Err(format!("error: {m}"))
                    }
                    Ok(conflicts) => {
                        self.conflict_lines(&conflicts);
                        self.long_status(g);
                        if sub == "pop" {
                            self.say(OUT, format!("Dropped refs/stash@{{{i}}} ({})", e.1.hex()));
                        }
                        Ok(())
                    }
                }
            }
            "drop" => {
                let i = pick(rest)?;
                let list = g.stashes();
                let Some(e) = list.iter().find(|x| x.0 == i).cloned() else {
                    return Err(if list.is_empty() { "No stash entries found.".into() } else { format!("error: stash@{{{i}}} is not a valid reference") });
                };
                g.stash_drop(i)?;
                self.say(OUT, format!("Dropped refs/stash@{{{i}}} ({})", e.1.hex()));
                Ok(())
            }
            "clear" => {
                while !g.stashes().is_empty() {
                    g.stash_drop(0)?;
                }
                Ok(())
            }
            "show" => {
                let i = pick(rest)?;
                let Some(e) = g.stashes().into_iter().find(|x| x.0 == i) else {
                    return Err("No stash entries found.".into());
                };
                let k = g.commit(&e.1)?;
                let patch = rest.iter().any(|x| x == "-p" || x == "--patch");
                let t = g.diff(2, k.parents.first(), Some(&e.1), if patch { 0 } else { 1 }, &[])?;
                for (kind, l) in crate::cmd::diff_lines(&t) {
                    self.say(if patch { kind } else { OUT }, l);
                }
                Ok(())
            }
            "branch" | "create" | "store" => Err(format!("git stash {sub} isn't part of these lessons.")),
            s => Err(format!("error: unknown subcommand: `{s}'")),
        }
    }

    // ---------- bisect ----------

    pub(crate) fn bisect_cmd(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let Some(&sub) = a.first() else {
            return Err("usage: git bisect [start|bad|good|skip|reset|run|log]".into());
        };
        let rest = &a[1..];
        if sub == "start" {
            if self.bis.is_some() {
                self.bisect_reset(g)?;
            }
            let orig = match g.head_branch().filter(|_| !g.detached()) {
                Some(b) => b,
                None => g.head().ok_or("fatal: no commits yet")?.hex(),
            };
            self.bis = Some(BisectState {
                good: vec![],
                bad: None,
                skip: vec![],
                orig,
                found: None,
                });
            let revs: Vec<&&str> = rest.iter().filter(|x| !x.starts_with('-')).collect();
            if let Some(b) = revs.first() {
                let o = self.rev(g, b)?;
                self.bis.as_mut().unwrap().bad = Some(o);
            }
            for r in revs.iter().skip(1) {
                let o = self.rev(g, r)?;
                self.bis.as_mut().unwrap().good.push(o);
            }
            return self.bisect_next(g);
        }
        if sub == "reset" {
            if self.bis.is_none() {
                self.say(OUT, "We are not bisecting.");
                return Ok(());
            }
            return self.bisect_reset(g);
        }
        if self.bis.is_none() {
            return Err("You need to start by \"git bisect start\"".into());
        }
        match sub {
            "bad" | "new" | "good" | "old" | "skip" => {
                let revs: Vec<Oid> = if rest.is_empty() {
                    vec![g.head().ok_or("fatal: no commits yet")?]
                } else {
                    rest.iter().map(|r| self.rev(g, r)).collect::<R<Vec<_>>>()?
                };
                let b = self.bis.as_mut().unwrap();
                for o in revs {
                    match sub {
                        "bad" | "new" => b.bad = Some(o),
                        "good" | "old" => b.good.push(o),
                        _ => b.skip.push(o),
                    }
                }
                self.bisect_next(g)
            }
            "run" => {
                let cmd = rest.join(" ");
                let ok_cmd = ["make test", "make check", "make", "./test", "test", "npm test"];
                if !ok_cmd.contains(&cmd.as_str()) {
                    return Err(format!("bisect run: the test here is make test, not '{cmd}'"));
                }
                if self.bis.as_ref().is_some_and(|b| b.bad.is_none() || b.good.is_empty()) {
                    return Err("bisect run: mark a good and a bad commit first, with git bisect good and git bisect bad".into());
                }
                for _ in 0..64 {
                    if self.bis.as_ref().is_some_and(|b| b.found.is_some()) {
                        self.say(GOOD, "bisect found first bad commit");
                        return Ok(());
                    }
                    self.say(OUT, format!("running '{cmd}'"));
                    let ok = self.test();
                    let h = g.head().unwrap();
                    let b = self.bis.as_mut().unwrap();
                    if ok {
                        b.good.push(h);
                    } else {
                        b.bad = Some(h);
                    }
                    self.bisect_next(g)?;
                }
                Ok(())
            }
            "log" => {
                let b = self.bis.clone().unwrap();
                self.say(OUT, "git bisect start");
                if let Some(o) = b.bad {
                    self.say(OUT, format!("# bad: [{}] {}\ngit bisect bad {}", o.hex(), summary(g, &o), o.hex()));
                }
                for o in &b.good {
                    self.say(OUT, format!("# good: [{}] {}\ngit bisect good {}", o.hex(), summary(g, o), o.hex()));
                }
                Ok(())
            }
            "visualize" | "view" => self.log(g, &["--oneline"]),
            s => Err(format!("error: unknown bisect command: {s}")),
        }
    }

    fn bisect_reset(&mut self, g: &G) -> R<()> {
        let b = self.bis.take().unwrap();
        let h = g.head();
        let to = match g.ref_id(&format!("refs/heads/{}", b.orig)) {
            Some(o) => o,
            None => Oid::parse(&b.orig).ok_or("error: bisect has nowhere to go back to")?,
        };
        if let Err(e) = self.checkout_files(g, &to, false, "checkout") {
            self.bis = Some(b);
            return Err(e);
        }
        if g.ref_id(&format!("refs/heads/{}", b.orig)).is_some() {
            g.set_head(&format!("refs/heads/{}", b.orig))?;
            if let Some(h) = h.filter(|h| *h != to) {
                self.say(OUT, format!("Previous HEAD position was {} {}", ab(g, &h), summary(g, &h)));
            }
            self.say(OUT, format!("Switched to branch '{}'", b.orig));
        } else {
            g.set_head_detached(&to)?;
        }
        Ok(())
    }

    /// Picks the next commit to test, or names the first bad one.
    fn bisect_next(&mut self, g: &G) -> R<()> {
        let b = self.bis.clone().unwrap();
        let Some(bad) = b.bad else {
            if b.good.is_empty() {
                self.say(OUT, "status: waiting for both good and bad commits");
            } else {
                self.say(OUT, "status: waiting for bad commit, 1 good commit known");
            }
            return Ok(());
        };
        if b.good.is_empty() {
            self.say(OUT, "status: waiting for good commit(s), bad commit known");
            return Ok(());
        }
        let cand = walk(g, &[bad], &b.good);
        if cand.is_empty() {
            self.bis.as_mut().unwrap().bad = None;
            return Err("Some good revs are not ancestors of the bad rev.\ngit bisect cannot work properly in this case.".into());
        }
        let n = cand.len();
        let skip: HashSet<Oid> = b.skip.iter().copied().collect();
        let testable: Vec<&lg::Commit> = cand.iter().filter(|c| c.id != bad && !skip.contains(&c.id)).collect();
        if n == 1 {
            return self.bisect_found(g, &bad);
        }
        if testable.is_empty() {
            self.say(OUT, "There are only 'skip'ped commits left to test.\nThe first bad commit could be any of:");
            for c in &cand {
                self.say(OUT, c.id.hex());
            }
            return Err("We cannot bisect more!".into());
        }
        let mut best = (0usize, testable[0].id);
        for c in &testable {
            let below = walk(g, &[c.id], &b.good).len();
            let score = below.min(n - below);
            if score > best.0 {
                best = (score, c.id);
            }
        }
        let next = best.1;
        let left = n / 2;
        let left = left.saturating_sub(1).max(if n > 2 { 1 } else { 0 });
        let steps = (usize::BITS - left.leading_zeros()) as usize;
        self.checkout_files(g, &next, false, "checkout")?;
        g.set_head_detached(&next)?;
        self.say(
            OUT,
            format!(
                "Bisecting: {} left to test after this (roughly {})\n[{}] {}",
                plural(left, "revision"),
                plural(steps, "step"),
                next.hex(),
                summary(g, &next)
            ),
        );
        Ok(())
    }

    fn bisect_found(&mut self, g: &G, bad: &Oid) -> R<()> {
        let k = g.commit(bad)?;
        self.bis.as_mut().unwrap().found = Some(*bad);
        self.say(GOOD, format!("{} is the first bad commit", bad.hex()));
        self.say(
            OUT,
            format!("commit {}\nAuthor: {} <{}>\nDate:   {}\n\n    {}\n", bad.hex(), k.author, k.email, crate::cmd::date(k.time), k.summary),
        );
        self.stat(g, k.parents.first(), bad);
        Ok(())
    }

    // ---------- worktrees ----------

    pub(crate) fn worktree(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let Some(sub) = a.first().cloned() else {
            return Err("usage: git worktree add <path> [<commit-ish>]\n   or: git worktree list\n   or: git worktree remove <worktree>".into());
        };
        let rest = &a[1..];
        let main = lg::open(&common_workdir(g))?;
        match sub.as_str() {
            "add" => {
                let mut new: Option<(String, bool)> = None;
                let mut detach = false;
                let mut pos = vec![];
                let mut i = 0;
                while i < rest.len() {
                    match rest[i].as_str() {
                        "-b" | "-B" => {
                            i += 1;
                            new = Some((rest.get(i).cloned().ok_or("error: switch `b' requires a value")?, rest[i - 1] == "-B"));
                        }
                        "--detach" | "-d" => detach = true,
                        "-f" | "--force" | "-q" | "--quiet" | "--checkout" => {}
                        s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                        s => pos.push(s.to_string()),
                    }
                    i += 1;
                }
                let Some(p) = pos.first() else {
                    return Err("usage: git worktree add [-b <new-branch>] <path> [<commit-ish>]".into());
                };
                let path = join(&self.cwd, p);
                let fs = vfs::cur();
                if fs.exists(&path) && fs.list(&path).map_or(true, |l| !l.is_empty()) {
                    return Err(format!("fatal: '{p}' already exists"));
                }
                let name = base_name(&path);
                if main.worktrees().iter().any(|w| w.0 == name) {
                    return Err(format!("fatal: a worktree named '{name}' is already there; pick another folder name"));
                }
                let start = pos.get(1).cloned();
                let at = match &start {
                    Some(s) => g.commit_of(s).map_err(|_| format!("fatal: invalid reference: {s}"))?,
                    None => g.head().ok_or("fatal: not a valid object name: 'HEAD'")?,
                };
                let local = start.as_ref().filter(|s| main.ref_id(&format!("refs/heads/{s}")).is_some()).cloned();
                if let Some((b, force)) = &new {
                    if main.ref_id(&format!("refs/heads/{b}")).is_some() && !force {
                        return Err(format!("fatal: a branch named '{b}' already exists"));
                    }
                    main.create_branch(b, &at, *force)?;
                    main.worktree_add(&name, &path, Some(&format!("refs/heads/{b}")))?;
                    self.say(OUT, format!("Preparing worktree (new branch '{b}')"));
                } else if detach || (start.is_some() && local.is_none()) {
                    let tmp = format!("bw-worktree-{name}");
                    main.create_branch(&tmp, &at, true)?;
                    main.worktree_add(&name, &path, Some(&format!("refs/heads/{tmp}")))?;
                    let w = lg::open(&path)?;
                    w.set_head_detached(&at)?;
                    drop(w);
                    main.delete_branch(&tmp, false)?;
                    self.say(OUT, format!("Preparing worktree (detached HEAD {})", ab(g, &at)));
                } else if let Some(b) = local {
                    if let Some(w) = self.checked_out_at(g, &b) {
                        return Err(format!("fatal: '{b}' is already used by worktree at '{w}'"));
                    }
                    main.worktree_add(&name, &path, Some(&format!("refs/heads/{b}")))?;
                    self.say(OUT, format!("Preparing worktree (checking out '{b}')"));
                } else {
                    if main.ref_id(&format!("refs/heads/{name}")).is_some() {
                        return Err(format!("fatal: a branch named '{name}' already exists"));
                    }
                    main.create_branch(&name, &at, false)?;
                    main.worktree_add(&name, &path, Some(&format!("refs/heads/{name}")))?;
                    self.say(OUT, format!("Preparing worktree (new branch '{name}')"));
                }
                self.say(OUT, format!("HEAD is now at {} {}", ab(g, &at), summary(g, &at)));
                Ok(())
            }
            "list" => {
                let mut rows = vec![(main.workdir(), main.head(), main.head_branch().filter(|_| !main.detached()))];
                for (_, p) in main.worktrees() {
                    if let Ok(w) = lg::open(&p) {
                        rows.push((p, w.head(), w.head_branch().filter(|_| !w.detached())));
                    }
                }
                let wide = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
                for (p, h, b) in rows {
                    let id = h.map(|h| ab(g, &h)).unwrap_or("0000".into());
                    let b = b.map(|b| format!("[{b}]")).unwrap_or("(detached HEAD)".into());
                    self.say(OUT, format!("{p:<wide$}  {id} {b}"));
                }
                Ok(())
            }
            "remove" => {
                let force = rest.iter().any(|x| x == "-f" || x == "--force");
                let Some(p) = rest.iter().find(|x| !x.starts_with('-')) else {
                    return Err("usage: git worktree remove [--force] <worktree>".into());
                };
                let path = join(&self.cwd, p);
                let Some((name, wpath)) = main.worktrees().into_iter().find(|w| w.1 == path || w.0 == *p) else {
                    if path == main.workdir() {
                        return Err(format!("fatal: '{p}' is a main working tree"));
                    }
                    return Err(format!("fatal: '{p}' is not a working tree"));
                };
                if self.cwd == wpath || self.cwd.starts_with(&format!("{wpath}/")) {
                    return Err("fatal: you are inside that worktree; cd out of it first".into());
                }
                if !force {
                    if let Ok(w) = lg::open(&wpath) {
                        if !w.status().is_empty() {
                            return Err(format!("fatal: '{p}' contains modified or untracked files, use --force to delete it"));
                        }
                    }
                }
                main.worktree_prune(&name)?;
                if vfs::cur().exists(&wpath) {
                    let _ = vfs::cur().remove(&wpath);
                }
                Ok(())
            }
            "prune" => {
                for (name, p) in main.worktrees() {
                    if !vfs::cur().exists(&p) {
                        main.worktree_prune(&name)?;
                    }
                }
                Ok(())
            }
            "move" | "lock" | "unlock" | "repair" => Err(format!("git worktree {sub} isn't part of these lessons.")),
            s => Err(format!("error: unknown subcommand: `{s}'")),
        }
    }

    // ---------- remotes ----------

    /// The remote a command means: the one named, the current branch's, or
    /// origin.
    fn remote_of(&self, g: &G, named: Option<&str>) -> R<String> {
        let all = g.remotes();
        if let Some(n) = named {
            if all.iter().any(|r| r == n) {
                return Ok(n.to_string());
            }
            return Err(format!("fatal: '{n}' does not appear to be a git repository\nfatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists."));
        }
        if let Some(b) = g.head_branch() {
            if let Some(r) = g.get_config(&format!("branch.{b}.remote")) {
                return Ok(r);
            }
        }
        if all.iter().any(|r| r == "origin") {
            return Ok("origin".into());
        }
        Err("fatal: No remote repository specified.  Please, specify either a URL or a\nremote name from which new revisions should be fetched.".into())
    }

    fn remote_refs(g: &G, remote: &str) -> HashMap<String, Oid> {
        g.refs()
            .into_iter()
            .filter(|r| r.starts_with(&format!("refs/remotes/{remote}/")) || r.starts_with("refs/tags/"))
            .filter(|r| !r.ends_with("/HEAD"))
            .filter_map(|r| g.ref_id(&r).map(|o| (r, o)))
            .collect()
    }

    pub(crate) fn fetch(&mut self, g: &G, a: &[&str]) -> R<()> {
        let prune = a.iter().any(|x| *x == "--prune" || *x == "-p") || g.get_bool("fetch.prune").unwrap_or(false);
        let tags = a.iter().any(|x| *x == "--tags" || *x == "-t");
        let all = a.iter().any(|x| *x == "--all");
        let named: Vec<&&str> = a.iter().filter(|x| !x.starts_with('-')).collect();
        if named.len() > 1 {
            return Err("fetching single branches isn't part of these lessons; use git fetch".into());
        }
        let remotes = if all {
            g.remotes()
        } else {
            vec![self.remote_of(g, named.first().map(|x| **x))?]
        };
        for remote in remotes {
            let url = g.remote_url(&remote).unwrap_or_default();
            if lg::open(&url).is_err() {
                return Err(format!("fatal: '{url}' does not appear to be a git repository\nfatal: Could not read from remote repository."));
            }
            let before = Self::remote_refs(g, &remote);
            g.fetch(&remote, prune, tags)?;
            let after = Self::remote_refs(g, &remote);
            let mut lines = vec![];
            let mut names: Vec<&String> = before.keys().chain(after.keys()).collect::<HashSet<_>>().into_iter().collect();
            names.sort();
            for r in names {
                let short = short_ref(r).to_string();
                let tag = r.starts_with("refs/tags/");
                let src = if tag { short.clone() } else { short.splitn(2, '/').nth(1).unwrap_or("").to_string() };
                match (before.get(r), after.get(r)) {
                    (None, Some(_)) => {
                        let what = if tag { "[new tag]   " } else { "[new branch]" };
                        lines.push(format!(" * {what}      {src:<10} -> {short}"));
                    }
                    (Some(o), Some(n)) if o != n => {
                        if g.descends(n, o) {
                            lines.push(format!("   {}..{}  {src:<10} -> {short}", ab(g, o), ab(g, n)));
                        } else {
                            lines.push(format!(" + {}...{} {src:<10} -> {short}  (forced update)", ab(g, o), ab(g, n)));
                        }
                    }
                    (Some(_), None) => lines.push(format!(" - [deleted]         (none)     -> {short}")),
                    _ => {}
                }
            }
            if !lines.is_empty() {
                self.say(OUT, format!("From {url}"));
                self.say(OUT, lines.join("\n"));
            }
        }
        Ok(())
    }

    pub(crate) fn pull(&mut self, g: &G, a: &[&str]) -> R<()> {
        if g.is_bare() {
            return Err("fatal: this operation must be run in a work tree".into());
        }
        let a = expand(a);
        let mut mode: Option<&str> = None;
        let mut pos = vec![];
        for w in &a {
            match w.as_str() {
                "--rebase" | "-r" => mode = Some("rebase"),
                "--no-rebase" => mode = Some("merge"),
                "--ff-only" => mode = Some("ff"),
                "--ff" | "--no-ff" | "-q" | "--quiet" | "-v" => {}
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => pos.push(s.to_string()),
            }
        }
        let Some(b) = g.head_branch().filter(|_| !g.detached()) else {
            return Err("You are not currently on a branch.\nPlease specify which branch you want to merge with.".into());
        };
        let (remote, up) = match pos.as_slice() {
            [r, br] => (self.remote_of(g, Some(r))?, format!("refs/remotes/{r}/{br}")),
            [r] => {
                let r = self.remote_of(g, Some(r))?;
                (r.clone(), format!("refs/remotes/{r}/{b}"))
            }
            _ => match g.upstream(&b) {
                Some(u) => (self.remote_of(g, None)?, u),
                None => {
                    return Err(format!("There is no tracking information for the current branch.\nPlease specify which branch you want to merge with.\n\n    git pull <remote> <branch>\n\nIf you wish to set tracking information for this branch you can do so with:\n\n    git branch --set-upstream-to=origin/{b} {b}"));
                }
            },
        };
        self.fetch(g, &[remote.as_str()])?;
        let short = short_ref(&up).to_string();
        let Some(target) = g.ref_id(&up) else {
            return Err(format!("fatal: couldn't find remote ref {}", short.splitn(2, '/').nth(1).unwrap_or(&short)));
        };
        let Some(head) = g.head() else {
            return self.merge(g, &[short.as_str()]);
        };
        if head == target || g.descends(&head, &target) {
            self.say(OUT, "Already up to date.");
            return Ok(());
        }
        if g.descends(&target, &head) {
            return self.merge(g, &[short.as_str()]);
        }
        let mode = mode.or_else(|| match g.get_bool("pull.rebase") {
            Some(true) => Some("rebase"),
            Some(false) => Some("merge"),
            None => (g.get_config("pull.ff").as_deref() == Some("only")).then_some("ff"),
        });
        match mode {
            Some("rebase") => self.rebase(g, &[short.as_str()]),
            Some("merge") => self.merge(g, &[short.as_str()]),
            Some(_) => Err("fatal: Not possible to fast-forward, aborting.".into()),
            None => {
                self.hint(DIVERGENT);
                Err("fatal: Need to specify how to reconcile divergent branches.".into())
            }
        }
    }

    pub(crate) fn push(&mut self, g: &G, a: &[&str]) -> R<()> {
        let a = expand(a);
        let mut set_up = false;
        let mut force = false;
        let mut lease = false;
        let mut tags = false;
        let mut delete = false;
        let mut pos: Vec<String> = vec![];
        for w in &a {
            match w.as_str() {
                "-u" | "--set-upstream" => set_up = true,
                "-f" | "--force" => force = true,
                "--force-with-lease" | "--force-if-includes" => lease = true,
                "--tags" => tags = true,
                "-d" | "--delete" => delete = true,
                "-q" | "--quiet" | "-v" | "--verbose" | "--no-verify" => {}
                s if s.starts_with("--force-with-lease") => lease = true,
                s if s.starts_with('-') => return Err(format!("error: unknown option `{}'", s.trim_start_matches('-'))),
                s => pos.push(s.to_string()),
            }
        }
        let remote = self.remote_of(g, pos.first().map(|s| s.as_str())).map_err(|e| {
            if g.remotes().is_empty() {
                "fatal: No configured push destination.\nEither specify the URL from the command-line or configure a remote repository using\n\n    git remote add <name> <url>\n\nand then push using the remote name\n\n    git push <name>".into()
            } else {
                e
            }
        })?;
        let url = g.remote_url(&remote).unwrap_or_default();
        let there = lg::open(&url).map_err(|_| {
            format!("fatal: '{url}' does not appear to be a git repository\nfatal: Could not read from remote repository.")
        })?;
        let branch = g.head_branch().filter(|_| !g.detached());
        // (local ref or "", remote ref, forced)
        let mut specs: Vec<(String, String, bool)> = vec![];
        let names: Vec<String> = pos.iter().skip(1).cloned().collect();
        if names.is_empty() && !tags {
            let Some(b) = branch.clone() else {
                return Err("fatal: You are not currently on a branch.\nTo push the history leading to the current (detached HEAD)\nstate now, use\n\n    git push origin HEAD:<name-of-remote-branch>".into());
            };
            let up = g.upstream(&b);
            if up.is_none() && !set_up && !g.get_bool("push.autoSetupRemote").unwrap_or(false) {
                return Err(format!("fatal: The current branch {b} has no upstream branch.\nTo push the current branch and set the remote as upstream, use\n\n    git push --set-upstream {remote} {b}\n\nTo have this happen automatically for branches without a tracking\nupstream, see 'push.autoSetupRemote' in 'git help config'."));
            }
            if up.is_none() {
                set_up = true;
            }
            let dst = up
                .as_deref()
                .and_then(|u| u.strip_prefix(&format!("refs/remotes/{remote}/")))
                .unwrap_or(&b)
                .to_string();
            specs.push((format!("refs/heads/{b}"), format!("refs/heads/{dst}"), force));
        }
        for n in &names {
            let (plus, n) = match n.strip_prefix('+') {
                Some(x) => (true, x),
                None => (false, n.as_str()),
            };
            if delete {
                specs.push((String::new(), format!("refs/heads/{}", n.trim_start_matches("refs/heads/")), true));
                continue;
            }
            let (src, dst) = match n.split_once(':') {
                Some((s, d)) => (s.to_string(), Some(d.to_string())),
                None => (n.to_string(), None),
            };
            if src.is_empty() {
                let d = dst.unwrap_or_default();
                specs.push((String::new(), if d.starts_with("refs/") { d } else { format!("refs/heads/{d}") }, true));
                continue;
            }
            let full = if src == "HEAD" {
                match &branch {
                    Some(b) => format!("refs/heads/{b}"),
                    None => "HEAD".into(),
                }
            } else if g.ref_id(&format!("refs/heads/{src}")).is_some() {
                format!("refs/heads/{src}")
            } else if g.ref_id(&format!("refs/tags/{src}")).is_some() {
                format!("refs/tags/{src}")
            } else if g.ref_id(&src).is_some() && src.starts_with("refs/") {
                src.clone()
            } else if g.commit_of(&src).is_ok() && dst.is_some() {
                src.clone()
            } else {
                return Err(format!("error: src refspec {src} does not match any\nerror: failed to push some refs to '{url}'"));
            };
            let dst = match dst {
                Some(d) if d.starts_with("refs/") => d,
                Some(d) => format!("{}{d}", if full.starts_with("refs/tags/") { "refs/tags/" } else { "refs/heads/" }),
                None if full.starts_with("refs/") => full.clone(),
                None => return Err(format!("error: The destination you provided is not a full refname: {src}")),
            };
            specs.push((full, dst, force || plus));
        }
        if tags {
            for r in g.refs() {
                if r.starts_with("refs/tags/") && !specs.iter().any(|s| s.1 == r) {
                    specs.push((r.clone(), r, false));
                }
            }
        }
        let mut lines = vec![];
        let mut send: Vec<String> = vec![];
        let mut rejected = vec![];
        let mut pushed_heads = vec![];
        for (src, dst, forced) in &specs {
            let d = short_ref(dst).to_string();
            let old = there.ref_id(dst);
            if src.is_empty() {
                match old {
                    None => {
                        rejected.push(format!("error: unable to delete '{d}': remote ref does not exist"));
                    }
                    Some(_) => {
                        lines.push(format!(" - [deleted]         {d}"));
                        send.push(format!(":{dst}"));
                    }
                }
                continue;
            }
            let new = g.commit_of(src).map_err(|e| e.1)?;
            let new_obj = g.ref_id(src).unwrap_or(new);
            let s = short_ref(src).to_string();
            let tag = dst.starts_with("refs/tags/");
            match old {
                Some(o) if o == new_obj => continue,
                None => {
                    let what = if tag { "[new tag]   " } else { "[new branch]" };
                    lines.push(format!(" * {what}      {s} -> {d}"));
                }
                Some(o) => {
                    let ff = !tag && g.commit(&o).is_ok() && g.descends(&new, &o);
                    let mine = g.ref_id(&format!("refs/remotes/{remote}/{d}"));
                    if ff && !lease {
                        lines.push(format!("   {}..{}  {s} -> {d}", ab(g, &o), ab(g, &new)));
                    } else if lease && mine != Some(o) {
                        lines.push(format!(" ! [rejected]        {s} -> {d} (stale info)"));
                        rejected.push("stale".to_string());
                        continue;
                    } else if *forced || lease {
                        if ff {
                            lines.push(format!("   {}..{}  {s} -> {d}", ab(g, &o), ab(g, &new)));
                        } else {
                            lines.push(format!(" + {}...{} {s} -> {d} (forced update)", ab(g, &o), ab(g, &new)));
                        }
                    } else if tag {
                        lines.push(format!(" ! [rejected]        {s} -> {d} (already exists)"));
                        rejected.push("tag".to_string());
                        continue;
                    } else if g.commit(&o).is_ok() {
                        lines.push(format!(" ! [rejected]        {s} -> {d} (non-fast-forward)"));
                        rejected.push("behind".to_string());
                        continue;
                    } else {
                        lines.push(format!(" ! [rejected]        {s} -> {d} (fetch first)"));
                        rejected.push("fetch".to_string());
                        continue;
                    }
                }
            }
            send.push(format!("+{src}:{dst}"));
            if dst.starts_with("refs/heads/") {
                pushed_heads.push((s.clone(), d.clone(), new));
            }
        }
        drop(there);
        if !send.is_empty() {
            let bad = g.push(&remote, &send)?;
            if let Some((r, why)) = bad.first() {
                return Err(format!("error: failed to push {r}: {why}"));
            }
        }
        // the remote-tracking branches follow what was sent
        for (src, dst, _) in &specs {
            if let Some(d) = dst.strip_prefix("refs/heads/") {
                let track = format!("refs/remotes/{remote}/{d}");
                if src.is_empty() {
                    if send.iter().any(|x| x == &format!(":{dst}")) && g.ref_id(&track).is_some() {
                        let _ = g.delete_ref(&track);
                    }
                } else if let Some((_, _, new)) = pushed_heads.iter().find(|p| p.1 == d) {
                    g.set_ref(&track, new, "update by push")?;
                }
            }
        }
        if lines.is_empty() && rejected.is_empty() {
            self.say(OUT, "Everything up-to-date");
        } else if !lines.is_empty() {
            self.say(OUT, format!("To {url}"));
            self.say(OUT, lines.join("\n"));
        }
        if set_up {
            for (s, d, _) in &pushed_heads {
                if g.ref_id(&format!("refs/heads/{s}")).is_some() {
                    g.set_upstream(s, Some(&format!("{remote}/{d}")))?;
                    self.say(OUT, format!("branch '{s}' set up to track '{remote}/{d}'."));
                }
            }
        }
        if !rejected.is_empty() {
            for r in &rejected {
                if r.starts_with("error:") {
                    self.err(r.clone());
                }
            }
            self.err(format!("error: failed to push some refs to '{url}'"));
            if rejected.iter().any(|r| r == "fetch") {
                self.hint(PULL_FIRST);
            } else if rejected.iter().any(|r| r == "behind") {
                self.hint(BEHIND);
            } else if rejected.iter().any(|r| r == "stale") {
                self.hint("hint: origin has commits you haven't fetched. Fetch and look at them before overwriting anything.");
            }
            return Err(String::new());
        }
        Ok(())
    }

    pub(crate) fn remote(&mut self, g: &G, a: &[&str]) -> R<()> {
        match a.first().copied() {
            None => {
                for r in g.remotes() {
                    self.say(OUT, r);
                }
                Ok(())
            }
            Some("-v") | Some("--verbose") => {
                for r in g.remotes() {
                    let u = g.remote_url(&r).unwrap_or_default();
                    self.say(OUT, format!("{r}\t{u} (fetch)\n{r}\t{u} (push)"));
                }
                Ok(())
            }
            Some("add") => {
                let w: Vec<&&str> = a[1..].iter().filter(|x| !x.starts_with('-')).collect();
                let [name, url] = w.as_slice() else {
                    return Err("usage: git remote add <name> <url>".into());
                };
                if g.remotes().iter().any(|r| r == **name) {
                    return Err(format!("error: remote {name} already exists."));
                }
                let url = if url.contains("://") || url.contains('@') { url.to_string() } else { join(&self.cwd, url) };
                g.add_remote(name, &url)
            }
            Some("remove") | Some("rm") => {
                let Some(name) = a.get(1) else {
                    return Err("usage: git remote remove <name>".into());
                };
                if !g.remotes().iter().any(|r| r == *name) {
                    return Err(format!("error: No such remote: '{name}'"));
                }
                g.remove_remote(name)
            }
            Some("get-url") => {
                let Some(name) = a.get(1) else {
                    return Err("usage: git remote get-url <name>".into());
                };
                match g.remote_url(name) {
                    Some(u) => {
                        self.say(OUT, u);
                        Ok(())
                    }
                    None => Err(format!("error: No such remote '{name}'")),
                }
            }
            Some("show") => {
                for r in g.remotes() {
                    let u = g.remote_url(&r).unwrap_or_default();
                    self.say(OUT, format!("* remote {r}\n  Fetch URL: {u}\n  Push  URL: {u}"));
                }
                Ok(())
            }
            Some(s) => Err(format!("git remote {s} isn't part of these lessons.")),
        }
    }
}

/// Moves each "fixup! X" (or "squash! X") commit to just after X.
fn autosquash_order(g: &G, list: Vec<(u8, Oid)>) -> Vec<(u8, Oid)> {
    let sums: Vec<String> = list.iter().map(|x| summary(g, &x.1)).collect();
    let mut out: Vec<(u8, Oid)> = vec![];
    let mut after: HashMap<usize, Vec<(u8, Oid)>> = HashMap::new();
    let mut targets: Vec<usize> = vec![];
    for (i, (act, o)) in list.iter().enumerate() {
        let s = &sums[i];
        let marked = ["fixup! ", "amend! ", "squash! "].iter().find_map(|p| s.strip_prefix(p).map(|r| (*p, r)));
        if let Some((p, rest)) = marked {
            let rest = rest.trim_start_matches("fixup! ").trim_start_matches("squash! ");
            if let Some(t) = (0..i).find(|&k| {
                !sums[k].starts_with("fixup! ")
                    && !sums[k].starts_with("squash! ")
                    && (sums[k] == rest || sums[k].starts_with(rest) || list[k].1.hex().starts_with(rest))
            }) {
                after.entry(t).or_default().push((if p == "squash! " { b's' } else { b'f' }, *o));
                continue;
            }
        }
        targets.push(i);
        let _ = act;
    }
    for i in targets {
        out.push(list[i]);
        if let Some(v) = after.remove(&i) {
            out.extend(v);
        }
    }
    out
}
